import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { logger } from '$lib/logger';

export interface AnalysisProgress {
	status: string;
	current?: number;
	total?: number;
	record_id?: string;
	error?: string;
	chunk_count?: number;
	msg?: string;
	progress?: number;
	engine?: string;
	step?: string;
}

export interface LogEntry {
	id: string;
	time: string;
	msg: string;
	type: 'info' | 'error' | 'success';
}

class AnalysisStore {
	status = $state('standby');
	currentRecordId = $state<string | null>(null);
	processedCount = $state(0);
	totalCount = $state(0);
	progress = $state(0);
	logs = $state<LogEntry[]>([]);
	busy = $state(false);

	ocrDownloadProgress = $state(0);
	ocrDownloadMsg = $state('');

	private unlisten: UnlistenFn | null = null;

	async init(isOpen: boolean, setOpen: (open: boolean) => void, onComplete?: () => void) {
		this.unlisten = await listen<AnalysisProgress>('analysis-progress', (event) => {
			const payload = event.payload;

			const ocrStatuses = [
				'initializing-batch',
				'batch-planning',
				'extracting-foundation',
				'foundation-indexed',
				'indexing-vector',
				'record-completed',
				'record-failed',
				'loading-ocr-engine'
			];

			if (ocrStatuses.includes(payload.status)) {
				if (
					payload.status === 'initializing-batch' ||
					(payload.status === 'extracting-foundation' && payload.current === 1)
				) {
					this.logs = [];
					this.progress = 0;
					this.processedCount = 0;
					this.totalCount = 0;
					this.currentRecordId = null;
				}
				if (!isOpen) {
					setOpen(true);
				}
				this.busy = true;
				this.status =
					payload.status === 'extracting-foundation'
						? 'processing'
						: payload.status === 'indexing-vector'
							? 'vectorizing'
							: payload.status;
			} else if (payload.status === 'completed') {
				if (this.busy) {
					this.status = 'completed';
					this.busy = false;
					this.addLog('Ingestion and Vector Indexing complete.', 'success');
					if (onComplete) onComplete();
				}
				return;
			} else if (payload.status === 'failed') {
				if (this.busy) {
					this.status = 'failed';
					this.busy = false;
					this.addLog(`Process aborted: ${payload.error}`, 'error');
				}
				return;
			} else {
				return;
			}

			this.processedCount = payload.current ?? this.processedCount;
			this.totalCount = payload.total ?? this.totalCount;
			this.currentRecordId = payload.record_id ?? this.currentRecordId;

			if (this.totalCount > 0) {
				this.progress = (this.processedCount / this.totalCount) * 100;
			}

			if (payload.status === 'batch-planning') {
				this.addLog(payload.msg || 'Organizing batch execution plan...', 'info');
			} else if (payload.status === 'initializing-batch') {
				this.addLog(payload.msg || 'Resolving ingestion pipeline settings...', 'info');
			} else if (payload.status === 'extracting-foundation') {
				if (payload.step) {
					this.addLog(`[OCR Trace] ${payload.step}`, 'info');
				} else {
					this.addLog(
						`Analyzing document structures for record: ${payload.record_id?.substring(0, 12)}...`,
						'info'
					);
				}
			} else if (payload.status === 'foundation-indexed') {
				let engineName = 'Neural Vision';
				if (payload.engine === 'pdf-digital') {
					engineName = 'PDF Digital';
				} else if (payload.engine === 'text-file') {
					engineName = 'Plaintext File';
				} else if (payload.engine) {
					engineName = payload.engine
						.split('-')
						.map((w) => w.charAt(0).toUpperCase() + w.slice(1))
						.join(' ');
				}
				this.addLog(`Foundation metadata mapped via ${engineName}.`, 'success');
			} else if (payload.status === 'indexing-vector') {
				const chunkMsg = payload.chunk_count
					? ` (${payload.chunk_count} semantic associations)`
					: '';
				this.addLog(`Chunking and embedding text vectors${chunkMsg}...`, 'info');
			} else if (payload.status === 'record-completed') {
				this.addLog(
					`Record ${payload.record_id?.substring(0, 8)} successfully secured in the vault.`,
					'success'
				);
			} else if (payload.status === 'record-failed') {
				this.addLog(
					`Failed to index record ${payload.record_id?.substring(0, 8)}: ${payload.error}`,
					'error'
				);
			} else if (payload.status === 'loading-ocr-engine') {
				this.ocrDownloadProgress = payload.progress ?? this.ocrDownloadProgress;
				this.ocrDownloadMsg = payload.msg ?? this.ocrDownloadMsg;
				if (payload.msg) {
					this.addLog(payload.msg, 'info');
				}
			}
		});
	}

	destroy() {
		if (this.unlisten) this.unlisten();
	}

	addLog(msg: string, type: 'info' | 'error' | 'success' = 'info') {
		if (this.logs.length > 0 && this.logs[0].msg === msg) {
			return;
		}
		const time = new Date().toLocaleTimeString([], {
			hour12: false,
			hour: '2-digit',
			minute: '2-digit',
			second: '2-digit'
		});
		this.logs = [{ id: crypto.randomUUID(), time, msg, type }, ...this.logs].slice(0, 100);
	}

	async startAnalysis() {
		if (this.busy) return;
		this.busy = true;
		this.status = 'initializing';
		this.progress = 0;
		this.processedCount = 0;
		this.ocrDownloadProgress = 0;
		this.ocrDownloadMsg = '';
		this.logs = [];
		this.addLog('Secure Ingestion Pipeline initialized...', 'info');
		this.addLog('Starting Optical Character Recognition (OCR) runtime...', 'info');
		this.addLog('Allocating vector engine indices...', 'info');

		try {
			const cmd = 'analyze_all_records';
			const count = await invoke<number>(cmd);
			this.totalCount = count;
			if (count === 0) {
				this.addLog('Zero pending records identified. Database is fully audited.', 'success');
				this.status = 'completed';
				this.busy = false;
				return;
			}
			this.addLog(`Batch Queued: ${count} target files registered for ingestion.`, 'info');
			this.status = 'processing';
		} catch (e) {
			this.addLog(`Ingestion failed to start: ${e}`, 'error');
			this.status = 'failed';
			this.busy = false;
		}
	}

	async abortAnalysis() {
		if (!this.busy) return;
		this.addLog('Sending abort signal to foundation engines...', 'error');
		try {
			await invoke('abort_analysis');
			this.addLog('Abort signal acknowledged. Winding down tasks...', 'error');
		} catch (e) {
			logger.error('Failed to abort analysis:', e);
		}
	}
}

export const analysisStore = new AnalysisStore();
