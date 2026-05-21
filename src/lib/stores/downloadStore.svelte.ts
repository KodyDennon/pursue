import { invoke } from '@tauri-apps/api/core';
import { addToast } from '$lib/toastStore';
import type { BulkDownloadReport } from '$lib/types';
import { logger } from '$lib/logger';
import { settingsStore } from './settingsStore.svelte';

class DownloadStore {
	activeJobId = $state<string | null>(null);
	report = $state<BulkDownloadReport | null>(null);
	polling = $state(false);
	downloading = $state(false);
	private pollInterval: any = null;

	async init(onComplete?: () => void) {
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
	}

	async startBulkDownload(onComplete?: () => void) {
		try {
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

	async cancelDownload() {
		if (!this.activeJobId) return;
		try {
			await invoke('cancel_bulk_download', { id: this.activeJobId });
		} catch (e) {
			logger.error('Failed to cancel download:', e);
		}
	}

	async fetchStatus(onComplete?: () => void) {
		if (!this.activeJobId) return;
		try {
			this.report = await invoke<BulkDownloadReport>('get_bulk_download_status', {
				id: this.activeJobId
			});

			if (
				this.report.items.some((i) => i.status === 'queued') &&
				!this.downloading &&
				this.report.job.status === 'running'
			) {
				this.runDownloadWorker();
			}

			if (
				this.report.job.status === 'completed' ||
				this.report.job.status === 'failed' ||
				this.report.job.status === 'cancelled' ||
				this.report.job.status === 'completed_with_errors'
			) {
				this.stopPolling();
				if (onComplete) onComplete();
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

		const queued = this.report.items.filter((i) => i.status === 'queued');
		if (queued.length === 0) return;

		this.downloading = true;

		for (const item of queued) {
			if (this.report.job.status === 'cancelled' || this.report.job.cancel_requested) break;

			try {
				if (!item.url) {
					await invoke('update_download_item_status', {
						itemId: item.id,
						status: 'failed',
						error: 'No document URL available'
					});
					continue;
				}

				await invoke('update_download_item_status', {
					itemId: item.id,
					status: 'downloading'
				});

				const bytes = await invoke<number[]>('proxy_fetch_url', {
					url: item.url
				});

				await invoke('ingest_downloaded_bytes', {
					jobId: this.activeJobId,
					itemId: item.id,
					recordId: item.record_id,
					url: item.url,
					bytes: bytes
				});
			} catch (e) {
				logger.error(`Failed to download ${item.title}:`, e);
				await invoke('update_download_item_status', {
					itemId: item.id,
					status: 'failed',
					error: String(e)
				});
			}

			await new Promise((r) => setTimeout(r, 500));
			this.report = await invoke<BulkDownloadReport>('get_bulk_download_status', {
				id: this.activeJobId
			});
		}

		this.downloading = false;
	}

	startPolling(onComplete?: () => void) {
		if (this.polling) return;
		this.polling = true;
		this.fetchStatus(onComplete);
		this.pollInterval = setInterval(() => this.fetchStatus(onComplete), 2000);
	}

	stopPolling() {
		this.polling = false;
		if (this.pollInterval) clearInterval(this.pollInterval);
	}
}

export const downloadStore = new DownloadStore();
