import { invoke } from '@tauri-apps/api/core';
import { addToast } from '$lib/stores/toastStore.svelte';
import type {
	BulkDownloadItem,
	BulkDownloadReport,
	DownloadJobWindow,
	RecordSummary
} from '$lib/types';
import { logger } from '$lib/logger';
import { settingsStore } from './settingsStore.svelte';

class DownloadStore {
	activeJobId = $state<string | null>(null);
	report = $state<BulkDownloadReport | null>(null);
	polling = $state(false);
	downloading = $state(false);
	itemProgress = $state<Record<string, { bytes: number; total: number | null }>>({});
	private pollInterval: ReturnType<typeof setInterval> | null = null;
	private worker: Worker | null = null;
	private workerJobId: string | null = null;

	private onCompleteHandler: (() => void | Promise<void>) | null = null;

	async init(onComplete?: () => void | Promise<void>) {
		if (onComplete) this.onCompleteHandler = onComplete;
		try {
			const latest = await invoke<BulkDownloadReport | null>('get_latest_download_job');
			if (latest) {
				this.activeJobId = latest.job.id;
				this.report = latest;
				this.startPolling(onComplete);
			}
		} catch (e) {
			logger.error('Failed to check for active job', e);
		}
	}

	destroy() {
		this.stopPolling();
		if (this.worker) {
			this.worker.postMessage({ type: 'cancel' });
			this.worker.terminate();
			this.worker = null;
		}
	}

	async checkDiskSpace(requiredBytes: number): Promise<boolean> {
		try {
			const diskSpace = await invoke<{ available_bytes: number; total_bytes: number }>(
				'get_disk_space_info'
			);
			if (diskSpace.available_bytes < requiredBytes) {
				const availableGb = (diskSpace.available_bytes / (1024 * 1024 * 1024)).toFixed(1);
				const requiredGb = (requiredBytes / (1024 * 1024 * 1024)).toFixed(1);
				addToast({
					type: 'error',
					message: `Insufficient disk space: ${availableGb}GB available, need at least ${requiredGb}GB.`
				});
				return false;
			}
			return true;
		} catch (e) {
			logger.error('Failed to check disk space:', e);
			return true;
		}
	}

	async startBulkDownload(onComplete?: () => void | Promise<void>) {
		try {
			if (!(await this.checkDiskSpace(5 * 1024 * 1024 * 1024))) return; // 5GB for bulk

			this.activeJobId = await invoke<string>('download_missing_records');
			this.startPolling(onComplete);
			addToast({
				type: 'info',
				message: 'Ingestion Agent initiated bulk collection.',
				duration: 3000
			});
		} catch (e) {
			addToast({ type: 'error', message: `Agent failed: ${e}` });
		}
	}

	async startRecordDownload(record: RecordSummary, onComplete?: () => void | Promise<void>) {
		try {
			if (!(await this.checkDiskSpace(1024 * 1024 * 1024))) return; // 1GB for single record

			this.activeJobId = await invoke<string>('queue_record_download', { id: record.id });
			this.startPolling(onComplete);
			addToast({
				type: 'info',
				message: 'Evidence retrieval queued.',
				duration: 3000
			});
		} catch (e) {
			addToast({ type: 'error', message: `Download failed: ${e}` });
			throw e;
		}
	}

	async cancelDownload() {
		if (!this.activeJobId) return;
		try {
			await invoke('cancel_bulk_download', { id: this.activeJobId });
			if (this.worker) this.worker.postMessage({ type: 'cancel', jobId: this.activeJobId });
		} catch (e) {
			logger.error('Failed to cancel download:', e);
		}
	}

	async fetchStatus(onComplete?: () => void | Promise<void>) {
		const handler = onComplete ?? this.onCompleteHandler ?? undefined;
		if (!this.activeJobId) return;
		try {
			this.report = await invoke<DownloadJobWindow>('get_download_job_window', {
				id: this.activeJobId,
				limit: 75,
				offset: 0
			});

			if (!this.downloading && this.report.job.status === 'running') {
				this.runDownloadWorker();
			}

			if (
				this.report.job.status === 'completed' ||
				this.report.job.status === 'failed' ||
				this.report.job.status === 'cancelled' ||
				this.report.job.status === 'completed_with_errors'
			) {
				this.stopPolling();
				if (handler) await handler();
				if (
					(this.report.job.status === 'completed' ||
						this.report.job.status === 'completed_with_errors') &&
					settingsStore.agentSettings.auto_analyze
				) {
					addToast({
						type: 'info',
						message: 'Downloads complete. Auto-starting neural extraction...',
						duration: 5000
					});
					await invoke('analyze_all_records');
				}
			}
		} catch (e) {
			logger.error('Poll failed', e);
			this.stopPolling();
		}
	}

	async runDownloadWorker() {
		if (this.downloading || !this.activeJobId || !this.report) return;

		this.downloading = true;

		try {
			const queued = await invoke<BulkDownloadItem[]>('get_next_download_items', {
				jobId: this.activeJobId,
				limit: 4
			});

			if (queued.length === 0) {
				this.downloading = false;
				return;
			}

			this.workerJobId = this.activeJobId;
			this.ensureWorker();
			this.worker?.postMessage({
				type: 'start',
				jobId: this.activeJobId,
				items: queued,
				concurrency: settingsStore.performanceMode ? 2 : 3
			});
		} catch (e) {
			this.downloading = false;
			logger.error('Failed to start download worker', e);
		}
	}

	private ensureWorker() {
		if (this.worker) return;
		this.worker = new Worker(new URL('../downloads/downloadWorker.ts', import.meta.url), {
			type: 'module'
		});
		this.worker.onmessage = (event) => this.handleWorkerMessage(event.data);
		this.worker.onerror = (event) => {
			logger.error('Download worker crashed', event.message);
			this.downloading = false;
			addToast({ type: 'error', message: `Download worker crashed: ${event.message}` });
		};
	}

	private async handleWorkerMessage(message: {
		type: string;
		id?: number;
		command?: string;
		args?: Record<string, unknown>;
		jobId?: string;
		itemId?: string;
		bytesDownloaded?: number;
		totalBytes?: number | null;
		error?: string;
	}) {
		if (message.type === 'host-call' && message.id && message.command) {
			try {
				const value = await invoke(message.command, message.args ?? {});
				this.worker?.postMessage({ type: 'host-result', id: message.id, ok: true, value });
			} catch (e) {
				this.worker?.postMessage({
					type: 'host-result',
					id: message.id,
					ok: false,
					error: String(e)
				});
			}
			return;
		}

		if (
			message.type === 'progress' &&
			message.itemId &&
			typeof message.bytesDownloaded === 'number'
		) {
			this.itemProgress[message.itemId] = {
				bytes: message.bytesDownloaded,
				total: message.totalBytes ?? null
			};
			return;
		}

		if (message.type === 'item-completed' || message.type === 'item-failed') {
			if (message.type === 'item-failed') logger.error('Download item failed', message.error);
			if (this.activeJobId) await this.fetchStatus();
			return;
		}

		if (message.type === 'idle') {
			this.downloading = false;
			if (this.workerJobId === this.activeJobId && this.activeJobId) {
				await this.fetchStatus();
				this.runDownloadWorker();
			}
		}
	}

	startPolling(onComplete?: () => void | Promise<void>) {
		if (onComplete) this.onCompleteHandler = onComplete;
		if (this.polling) return;
		this.polling = true;
		this.fetchStatus(this.onCompleteHandler ?? undefined);
		this.pollInterval = setInterval(
			() => this.fetchStatus(this.onCompleteHandler ?? undefined),
			2000
		);
	}

	stopPolling() {
		this.polling = false;
		if (this.pollInterval) clearInterval(this.pollInterval);
		this.pollInterval = null;
	}
}

export const downloadStore = new DownloadStore();
