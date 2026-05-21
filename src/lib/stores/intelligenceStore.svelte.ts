import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { DatabaseStatus } from '$lib/types';
import { addToast } from '$lib/toastStore';
import { logger } from '$lib/logger';

export interface HardwareDiagnosticsType {
	cpu_brand: string;
	total_memory_gb: number;
	gpu_acceleration_available: boolean;
	recommended_tier: 'Standard' | 'Elite';
}

export interface IntelligenceModel {
	id: string;
	name: string;
	type: string;
	size: string;
	status: string;
	progress: number;
	url: string;
	filename: string;
	speedMbps: number | null;
	etaSeconds: number | null;
}

export interface EvidenceStats {
	total_count: number;
	local_count: number;
	total_size: number;
	pending_count: number;
	indexed_count: number;
	completed_count: number;
	unanalyzed_count: number;
}

class IntelligenceStore {
	status = $state<DatabaseStatus | null>(null);
	diagnostics = $state<HardwareDiagnosticsType | null>(null);
	evidenceStats = $state<EvidenceStats | null>(null);
	models = $state<IntelligenceModel[]>([]);
	busyModelId = $state<string | null>(null);

	analysisProgress = $state(0);
	analysisActive = $state(false);
	analysisStatus = $state('');
	processedCount = $state(0);
	totalCount = $state(0);

	runtimeProvisioned = $state(false);
	runtimeBusy = $state(false);

	private intervalId: any;
	private unlistenProgress: UnlistenFn | null = null;
	private unlistenAnalysis: UnlistenFn | null = null;

	async init() {
		await this.loadStatus();
		this.intervalId = setInterval(() => this.loadStatus(), 5000);

		this.unlistenProgress = await listen<{
			model_id: string;
			status: string;
			total_bytes?: number;
			bytes_downloaded: number;
			speed_mbps?: number;
			eta_seconds?: number;
		}>('model-progress', (event) => {
			const payload = event.payload;
			const model = this.models.find((m) => m.id === payload.model_id);
			if (model) {
				model.status = payload.status;
				if (payload.total_bytes) {
					model.progress = (payload.bytes_downloaded / payload.total_bytes) * 100;
				}
				model.speedMbps = payload.speed_mbps !== undefined ? payload.speed_mbps : null;
				model.etaSeconds = payload.eta_seconds !== undefined ? payload.eta_seconds : null;
			}
		});

		this.unlistenAnalysis = await listen<{
			current?: number;
			total?: number;
			status: string;
			token_index?: number;
			token_limit?: number;
			progress?: number;
			msg?: string;
		}>('analysis-progress', (event) => {
			const { current, total, status, progress, msg } = event.payload;

			this.processedCount = current ?? this.processedCount;
			this.totalCount = total ?? this.totalCount;

			if (status === 'completed' || status === 'batch-complete') {
				this.analysisActive = false;
				this.analysisStatus = 'Intelligence Standby';
				this.loadStatus();
			} else if (status === 'loading-model') {
				this.analysisActive = true;
				this.analysisStatus = msg || 'Initializing Neural Engine...';
				this.analysisProgress = progress ?? 0;
			} else if (status === 'synthesizing' || status === 'synthesizing-start') {
				this.analysisActive = true;
				const curToken = event.payload.token_index ?? 0;
				const totToken = event.payload.token_limit ?? 2048;
				this.analysisStatus = `Neural Synthesis: Auditing Artifact...`;
				this.analysisProgress = (curToken / totToken) * 100;
			} else {
				this.analysisActive = true;
				const cur = this.processedCount;
				const tot = this.totalCount;
				if (tot > 0) {
					this.analysisProgress = (cur / tot) * 100;
				}

				if (status === 'extracting-foundation') {
					this.analysisStatus = 'Foundation OCR In Progress...';
				} else if (status === 'indexing-vector') {
					this.analysisStatus = 'Vectorizing Semantic Chunks...';
				} else {
					this.analysisStatus = `Batch Process: ${cur} of ${tot}`;
				}
			}
		});
	}

	destroy() {
		if (this.intervalId) clearInterval(this.intervalId);
		if (this.unlistenProgress) this.unlistenProgress();
		if (this.unlistenAnalysis) this.unlistenAnalysis();
	}

	async loadStatus() {
		try {
			this.status = await invoke<DatabaseStatus>('get_database_status');
			this.diagnostics = await invoke<HardwareDiagnosticsType>('get_hardware_diagnostics');
			this.evidenceStats = await invoke<EvidenceStats>('get_evidence_stats');
			this.runtimeProvisioned = await invoke<boolean>('check_neural_runtime_status');

			if (this.models.length === 0) {
				const registry = await invoke<{
					id: string;
					name: string;
					model_type: string;
					size_label: string;
					repo_id: string;
					filename: string;
				}[]>('get_model_registry');
				this.models = registry.map((m) => ({
					...m,
					type: m.model_type.charAt(0).toUpperCase() + m.model_type.slice(1),
					size: m.size_label,
					status: 'pending',
					progress: 0,
					url: m.repo_id,
					speedMbps: null,
					etaSeconds: null
				}));
			}

			const modelStatus = await invoke<Record<string, boolean>>('check_model_status');
			for (const model of this.models) {
				model.status = modelStatus[model.id]
					? 'ready'
					: this.busyModelId === model.id
						? 'downloading'
						: 'missing';
			}
		} catch (e) {
			logger.error('Failed to load status:', e);
		}
	}

	async provisionRuntime() {
		if (this.runtimeBusy) return;
		this.runtimeBusy = true;
		try {
			addToast({ type: 'info', message: 'Provisioning Neural Vision Runtime...', duration: 3000 });
			await invoke('provision_neural_runtime');
			await this.loadStatus();
			addToast({ type: 'success', message: 'Neural Vision Runtime is ready.', duration: 3000 });
		} catch (e) {
			addToast({ type: 'error', message: `Provisioning failed: ${e}` });
		} finally {
			this.runtimeBusy = false;
		}
	}

	async downloadModel(modelId: string) {
		this.busyModelId = modelId;
		const model = this.models.find((m) => m.id === modelId);
		if (!model) return;

		try {
			addToast({ type: 'info', message: `Provisioning ${model.name}...`, duration: 3000 });
			await invoke('provision_model', { id: model.id, url: model.url, name: model.filename });
			await this.loadStatus();
			addToast({ type: 'success', message: `${model.name} is ready.`, duration: 3000 });
		} catch (e) {
			addToast({ type: 'error', message: `Provisioning failed: ${e}` });
		} finally {
			this.busyModelId = null;
		}
	}

	async provisionAll() {
		const missing = this.models.filter((m) => m.status === 'missing');
		for (const model of missing) {
			await this.downloadModel(model.id);
		}
	}

	async reindexAll(onAnalyze?: () => void) {
		if (this.analysisActive) return;
		try {
			if (onAnalyze) onAnalyze();
			this.analysisActive = true;
			this.analysisProgress = 0;
			this.processedCount = 0;
			this.totalCount = 0;
			this.analysisStatus = 'Scanning for pending audits...';
			const count = await invoke<number>('analyze_all_records');
			addToast({
				type: 'info',
				message: `Foundation Indexing initiated for ${count} records.`,
				duration: 5000
			});
		} catch (e) {
			addToast({ type: 'error', message: `Indexing failed: ${e}`, duration: 5000 });
			this.analysisActive = false;
		}
	}

	async forceReprocessAll(onAnalyze?: () => void) {
		if (this.analysisActive) return;
		if (
			!confirm(
				'CRITICAL ACTION: This will purge all existing intelligence, OCR results, and forensic summaries and rerun the entire foundation indexing pipeline. Proceed?'
			)
		)
			return;

		try {
			if (onAnalyze) onAnalyze();
			this.analysisActive = true;
			this.analysisProgress = 0;
			this.processedCount = 0;
			this.totalCount = 0;
			this.analysisStatus = 'Resetting Archive Intelligence...';
			const count = await invoke<number>('reprocess_all_records');
			addToast({
				type: 'info',
				message: `Deep Re-Audit initiated for ${count} records.`,
				duration: 5000
			});
		} catch (e) {
			addToast({ type: 'error', message: `Re-Audit failed: ${e}`, duration: 5000 });
			this.analysisActive = false;
		}
	}

	async runBatchSynthesis(onSynthesize?: () => void) {
		if (this.analysisActive) return;
		if (
			!confirm(
				'CRITICAL ACTION: This will invoke local Gemma 4 LLM inference sequentially across all records that have completed foundation processing but have not yet been synthesized. This is extremely resource-intensive. Proceed?'
			)
		)
			return;

		try {
			if (onSynthesize) onSynthesize();
			this.analysisActive = true;
			this.analysisProgress = 0;
			this.processedCount = 0;
			this.totalCount = 0;
			this.analysisStatus = 'Waking Neural Engine...';
			const count = await invoke<number>('synthesize_all_records');
			addToast({
				type: 'info',
				message: `Neural Synthesis initiated for ${count} records.`,
				duration: 5000
			});
		} catch (e) {
			addToast({ type: 'error', message: `Synthesis failed: ${e}`, duration: 5000 });
			this.analysisActive = false;
		}
	}
}

export const intelligenceStore = new IntelligenceStore();
