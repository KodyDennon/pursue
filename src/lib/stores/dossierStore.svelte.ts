import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { addToast } from '$lib/toastStore';
import { logger } from '$lib/logger';
import type {
	AnalysisReport,
	RecordSummary,
	RecordAsset,
	RecordForensics,
	IntelligenceLog,
	AnalysisChunk
} from '$lib/types';

class DossierStore {
	record = $state<RecordSummary | null>(null);
	analysis = $state<AnalysisReport | null>(null);
	forensics = $state<RecordForensics[]>([]);
	intelLogs = $state<IntelligenceLog[]>([]);
	chunks = $state<AnalysisChunk[]>([]);
	busy = $state<string | null>(null);
	error = $state<string | null>(null);

	analysisStatus = $state<string | null>(null);
	analysisProgress = $state(0);

	private unlisten: UnlistenFn | null = null;

	async init(record: RecordSummary) {
		this.record = record;
		await this.loadAnalysis();

		this.unlisten = await listen<{
			record_id: string;
			status: string;
			token_index?: number;
			token_text?: string;
		}>('analysis-progress', (event) => {
			const payload = event.payload;
			if (payload.record_id === this.record?.id) {
				if (payload.status === 'synthesizing' || payload.status === 'synthesizing-start') {
					this.analysisStatus = 'Neural Synthesis In Progress...';
					if (payload.token_index) {
						this.analysisProgress = Math.round((payload.token_index / 2048) * 100);
					}
				} else if (payload.status === 'loading-model') {
					this.analysisStatus = 'Waking Neural Engine...';
				} else if (payload.status === 'extracting-foundation') {
					this.analysisStatus = 'Foundation OCR In Progress...';
					this.analysisProgress = 20;
				}
			}
		});
	}

	destroy() {
		if (this.unlisten) this.unlisten();
	}

	async loadAnalysis() {
		if (!this.record) return;
		this.error = null;
		try {
			this.analysis = await invoke<AnalysisReport | null>('get_analysis_result', {
				id: this.record.id
			});

			if (this.record.analysis_status === 'completed' || this.record.analysis_status === 'indexed') {
				await Promise.all([this.loadForensics(), this.loadChunks()]);
			}
		} catch (e) {
			this.error = String(e);
		}
	}

	async loadForensics() {
		if (!this.record) return;
		try {
			this.forensics = await invoke<RecordForensics[]>('get_forensic_report', { id: this.record.id });
			this.intelLogs = await invoke<IntelligenceLog[]>('get_intelligence_logs', {
				id: this.record.id
			});
		} catch (e) {
			logger.error('Forensic load failed:', e);
		}
	}

	async loadChunks() {
		if (!this.record) return;
		try {
			this.chunks = await invoke<AnalysisChunk[]>('get_record_chunks', { id: this.record.id });
		} catch (e) {
			logger.error('Chunk load failed:', e);
		}
	}

	async download(onChanged?: () => void | Promise<void>) {
		if (!this.record) return;
		this.busy = 'download';
		this.error = null;
		try {
			if (!this.record.document_url) throw new Error('No source URL available');

			const bytes = await invoke<number[]>('proxy_fetch_url', {
				url: this.record.document_url
			});

			await invoke('download_record_with_bytes', {
				id: this.record.id,
				url: this.record.document_url,
				bytes: bytes
			});

			if (onChanged) await onChanged();
			await this.loadAnalysis();
			addToast({ type: 'success', message: 'Evidence retrieved and vaulted.', duration: 3000 });
		} catch (e) {
			this.error = String(e);
			addToast({ type: 'error', message: `Download failed: ${e}` });
		} finally {
			this.busy = null;
		}
	}

	async runFoundationIndexing(onChanged?: () => void | Promise<void>, onAnalyze?: () => void) {
		if (!this.record) return;
		this.busy = 'indexing';
		this.error = null;
		try {
			if (onAnalyze) onAnalyze();
			await invoke('index_record', { id: this.record.id, current: 1, total: 1 });
			if (onChanged) await onChanged();
			await this.loadAnalysis();
			addToast({ type: 'success', message: 'Foundation Indexed Successfully', duration: 2000 });
		} catch (e) {
			this.error = String(e);
		} finally {
			this.busy = null;
		}
	}

	async runDeepSynthesis(onChanged?: () => void | Promise<void>, onSynthesize?: () => void) {
		if (!this.record) return;
		this.busy = 'synthesis';
		this.error = null;
		try {
			if (onSynthesize) onSynthesize();
			const report = await invoke<AnalysisReport>('synthesize_intelligence', { id: this.record.id });
			this.analysis = report;
			if (onChanged) await onChanged();
			await this.loadForensics();
			addToast({ type: 'success', message: 'Intelligence Synthesis Complete', duration: 3000 });
		} catch (e) {
			this.error = String(e);
		} finally {
			this.busy = null;
		}
	}

	async revealLocal() {
		if (!this.record?.local_path) return;
		this.busy = 'open-path';
		try {
			const path = await invoke<string>('get_record_artifact_path', { id: this.record.id });
			await invoke('open_path', { path }); // Using invoke instead of plugin-opener for consistency if needed, but plugin-opener is fine too.
		} catch (e) {
			this.error = String(e);
		} finally {
			this.busy = null;
		}
	}
}

export const dossierStore = new DossierStore();
