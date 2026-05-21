<script lang="ts">
	import { invoke } from '@tauri-apps/api/core';
	import { onMount } from 'svelte';
	import { Brain, Loader2 } from 'lucide-svelte';
	import type { DatabaseStatus } from '$lib/types';
	import { addToast } from '$lib/toastStore';
	import { logger } from '$lib/logger';

	import { listen, type UnlistenFn } from '@tauri-apps/api/event';

	import HardwareDiagnostics from './intelligence_center/HardwareDiagnostics.svelte';
	import ModelManager from './intelligence_center/ModelManager.svelte';
	import VectorAnalytics from './intelligence_center/VectorAnalytics.svelte';
	import CognitiveSynthesis from './intelligence_center/CognitiveSynthesis.svelte';

	let { onAnalyze, onSynthesize } = $props<{
		onAnalyze?: () => void;
		onSynthesize?: () => void;
	}>();

	interface HardwareDiagnosticsType {
		cpu_brand: string;
		total_memory_gb: number;
		gpu_acceleration_available: boolean;
		recommended_tier: 'Standard' | 'Elite';
	}

	interface IntelligenceModel {
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

	interface EvidenceStats {
		total_count: number;
		local_count: number;
		total_size: number;
		pending_count: number;
		indexed_count: number;
		completed_count: number;
		unanalyzed_count: number;
	}

	let status = $state<DatabaseStatus | null>(null);
	let diagnostics = $state<HardwareDiagnosticsType | null>(null);
	let evidenceStats = $state<EvidenceStats | null>(null);
	let models = $state<IntelligenceModel[]>([]);
	let busyModelId = $state<string | null>(null);

	let analysisProgress = $state(0);
	let analysisActive = $state(false);
	let analysisStatus = $state('');
	let processedCount = $state(0);
	let totalCount = $state(0);

	let runtimeProvisioned = $state(false);
	let runtimeBusy = $state(false);

	async function loadStatus() {
		logger.debug('[IntelligenceCenter] Syncing status...');
		try {
			status = await invoke<DatabaseStatus>('get_database_status');
			diagnostics = await invoke<HardwareDiagnosticsType>('get_hardware_diagnostics');
			evidenceStats = await invoke<EvidenceStats>('get_evidence_stats');
			runtimeProvisioned = await invoke<boolean>('check_neural_runtime_status');
			logger.debug('[IntelligenceCenter] Diagnostics loaded:', diagnostics);
			if (models.length === 0) {
				const registry = await invoke<{
					id: string;
					name: string;
					model_type: string;
					size_label: string;
					repo_id: string;
					filename: string;
				}[]>('get_model_registry');
				models = registry.map((m) => ({
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
			models = models.map((m) => ({
				...m,
				status: modelStatus[m.id] ? 'ready' : busyModelId === m.id ? 'downloading' : 'missing'
			}));
		} catch (e) {
			console.error(e);
		}
	}

	async function provisionRuntime() {
		if (runtimeBusy) return;
		runtimeBusy = true;
		try {
			addToast({ type: 'info', message: 'Provisioning Neural Vision Runtime...', duration: 3000 });
			await invoke('provision_neural_runtime');
			await loadStatus();
			addToast({ type: 'success', message: 'Neural Vision Runtime is ready.', duration: 3000 });
		} catch (e) {
			addToast({ type: 'error', message: `Provisioning failed: ${e}` });
		} finally {
			runtimeBusy = false;
		}
	}

	async function downloadModel(modelId: string) {
		busyModelId = modelId;
		const model = models.find((m) => m.id === modelId);
		if (!model) return;

		try {
			addToast({ type: 'info', message: `Provisioning ${model.name}...`, duration: 3000 });
			await invoke('provision_model', { id: model.id, url: model.url, name: model.filename });
			await loadStatus();
			addToast({ type: 'success', message: `${model.name} is ready.`, duration: 3000 });
		} catch (e) {
			addToast({ type: 'error', message: `Provisioning failed: ${e}` });
		} finally {
			busyModelId = null;
		}
	}

	async function provisionAll() {
		const missing = models.filter((m) => m.status === 'missing');
		for (const model of missing) {
			await downloadModel(model.id);
		}
	}

	async function reindexAll() {
		if (analysisActive) return;
		try {
			if (onAnalyze) onAnalyze();
			analysisActive = true;
			analysisProgress = 0;
			processedCount = 0;
			totalCount = 0;
			analysisStatus = 'Scanning for pending audits...';
			const count = await invoke<number>('analyze_all_records');
			addToast({
				type: 'info',
				message: `Foundation Indexing initiated for ${count} records.`,
				duration: 5000
			});
		} catch (e) {
			addToast({ type: 'error', message: `Indexing failed: ${e}`, duration: 5000 });
			analysisActive = false;
		}
	}

	async function forceReprocessAll() {
		if (analysisActive) return;
		if (
			!confirm(
				'CRITICAL ACTION: This will purge all existing intelligence, OCR results, and forensic summaries and rerun the entire foundation indexing pipeline. Proceed?'
			)
		)
			return;

		try {
			if (onAnalyze) onAnalyze();
			analysisActive = true;
			analysisProgress = 0;
			processedCount = 0;
			totalCount = 0;
			analysisStatus = 'Resetting Archive Intelligence...';
			const count = await invoke<number>('reprocess_all_records');
			addToast({
				type: 'info',
				message: `Deep Re-Audit initiated for ${count} records.`,
				duration: 5000
			});
		} catch (e) {
			addToast({ type: 'error', message: `Re-Audit failed: ${e}`, duration: 5000 });
			analysisActive = false;
		}
	}

	async function runBatchSynthesis() {
		if (analysisActive) return;
		if (
			!confirm(
				'CRITICAL ACTION: This will invoke local Gemma 4 LLM inference sequentially across all records that have completed foundation processing but have not yet been synthesized. This is extremely resource-intensive. Proceed?'
			)
		)
			return;

		try {
			if (onSynthesize) onSynthesize();
			analysisActive = true;
			analysisProgress = 0;
			processedCount = 0;
			totalCount = 0;
			analysisStatus = 'Waking Neural Engine...';
			const count = await invoke<number>('synthesize_all_records');
			addToast({
				type: 'info',
				message: `Neural Synthesis initiated for ${count} records.`,
				duration: 5000
			});
		} catch (e) {
			addToast({ type: 'error', message: `Synthesis failed: ${e}`, duration: 5000 });
			analysisActive = false;
		}
	}

	onMount(() => {
		logger.debug('[IntelligenceCenter] Mounted.');
		loadStatus();
		const interval = setInterval(loadStatus, 5000);

		let unlistenProgress: UnlistenFn;
		let unlistenAnalysis: UnlistenFn;

		listen<{
			model_id: string;
			status: string;
			total_bytes?: number;
			bytes_downloaded: number;
			speed_mbps?: number;
			eta_seconds?: number;
		}>('model-progress', (event) => {
			const payload = event.payload;
			logger.debug('[IntelligenceCenter] Model Progress:', payload.status, payload.model_id);
			const model = models.find((m) => m.id === payload.model_id);
			if (model) {
				model.status = payload.status;
				if (payload.total_bytes) {
					model.progress = (payload.bytes_downloaded / payload.total_bytes) * 100;
				}
				model.speedMbps = payload.speed_mbps !== undefined ? payload.speed_mbps : null;
				model.etaSeconds = payload.eta_seconds !== undefined ? payload.eta_seconds : null;
			}
		}).then((u) => (unlistenProgress = u));

		listen<{
			current?: number;
			total?: number;
			status: string;
			token_index?: number;
			token_limit?: number;
			progress?: number;
			msg?: string;
		}>('analysis-progress', (event) => {
			const { current, total, status, progress, msg } = event.payload;
			logger.debug('[IntelligenceCenter] Analysis Progress:', status, { current, total, progress });

			processedCount = current ?? processedCount;
			totalCount = total ?? totalCount;

			if (status === 'completed' || status === 'batch-complete') {
				analysisActive = false;
				analysisStatus = 'Intelligence Standby';
				loadStatus();
			} else if (status === 'loading-model') {
				analysisActive = true;
				analysisStatus = msg || 'Initializing Neural Engine...';
				analysisProgress = progress ?? 0;
			} else if (status === 'synthesizing' || status === 'synthesizing-start') {
				analysisActive = true;
				const curToken = event.payload.token_index ?? 0;
				const totToken = event.payload.token_limit ?? 2048;
				analysisStatus = `Neural Synthesis: Auditing Artifact...`;
				analysisProgress = (curToken / totToken) * 100;
			} else {
				analysisActive = true;
				const cur = processedCount;
				const tot = totalCount;
				if (tot > 0) {
					analysisProgress = (cur / tot) * 100;
				}

				if (status === 'extracting-foundation') {
					analysisStatus = 'Foundation OCR In Progress...';
				} else if (status === 'indexing-vector') {
					analysisStatus = 'Vectorizing Semantic Chunks...';
				} else {
					analysisStatus = `Batch Process: ${cur} of ${tot}`;
				}
			}
		}).then((u) => (unlistenAnalysis = u));

		return () => {
			clearInterval(interval);
			if (unlistenProgress) unlistenProgress();
			if (unlistenAnalysis) unlistenAnalysis();
		};
	});
</script>

{#if !status || !diagnostics}
	<div class="center-loader">
		<Loader2 class="spin" size={32} />
		<span>Syncing Neural Engine...</span>
	</div>
{:else}
	<div class="intelligence-center custom-scrollbar">
		<header class="page-header">
			<div class="title-wrap">
				<Brain class="accent-icon" size={32} />
				<div>
					<h1>Neural Engine</h1>
					<p>Coordinate neural models, vector indices, and hardware acceleration.</p>
				</div>
			</div>
		</header>

		<div class="center-grid">
			<HardwareDiagnostics {diagnostics} />

			<ModelManager
				{models}
				{runtimeProvisioned}
				{runtimeBusy}
				{busyModelId}
				onProvisionRuntime={provisionRuntime}
				onDownloadModel={downloadModel}
				onProvisionAll={provisionAll}
			/>

			<VectorAnalytics
				{status}
				{analysisActive}
				{analysisStatus}
				{analysisProgress}
				onReindexAll={reindexAll}
				onForceReprocessAll={forceReprocessAll}
			/>

			<CognitiveSynthesis
				{evidenceStats}
				{analysisActive}
				{analysisStatus}
				{analysisProgress}
				onRunBatchSynthesis={runBatchSynthesis}
			/>
		</div>
	</div>
{/if}

<style>
	.center-loader {
		height: 100%;
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: 16px;
		color: var(--text-secondary);
		font-size: 14px;
		font-weight: 500;
	}
	.intelligence-center {
		height: 100%;
		overflow-y: auto;
		padding: 40px;
		display: flex;
		flex-direction: column;
		gap: 40px;
		background: var(--bg-base);
		animation: fadeIn 0.4s ease-out;
	}

	@keyframes fadeIn {
		from {
			opacity: 0;
			transform: translateY(10px);
		}
		to {
			opacity: 1;
			transform: translateY(0);
		}
	}

	.page-header {
		display: flex;
		justify-content: space-between;
		align-items: flex-end;
	}

	.title-wrap {
		display: flex;
		gap: 20px;
		align-items: center;
	}

	.title-wrap h1 {
		font-size: 32px;
		margin: 0;
		font-weight: 700;
	}

	.title-wrap p {
		color: var(--text-secondary);
		margin: 4px 0 0 0;
	}

	.accent-icon {
		color: var(--accent-primary);
	}

	.center-grid {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(400px, 1fr));
		gap: 24px;
	}

	:global(.spin) {
		animation: spin 1s linear infinite;
	}
	@keyframes spin {
		from {
			transform: rotate(0deg);
		}
		to {
			transform: rotate(360deg);
		}
	}
</style>
