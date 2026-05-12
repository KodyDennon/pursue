<script lang="ts">
	import { invoke } from '@tauri-apps/api/core';
	import { onMount } from 'svelte';
	import {
		Brain,
		Cpu,
		Database,
		HardDrive,
		Download,
		CheckCircle2,
		Loader2,
		RefreshCw,
		Zap
	} from 'lucide-svelte';
	import type { DatabaseStatus } from '$lib/types';
	import { addToast } from '$lib/toastStore';

	import { listen, type UnlistenFn } from '@tauri-apps/api/event';

	let { onAnalyze } = $props<{ onAnalyze?: () => void }>();

	interface HardwareDiagnostics {
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

	let status = $state<DatabaseStatus | null>(null);
	let diagnostics = $state<HardwareDiagnostics | null>(null);
	let models = $state<IntelligenceModel[]>([]);
	let busyModelId = $state<string | null>(null);

	let analysisProgress = $state(0);
	let analysisActive = $state(false);
	let analysisStatus = $state('');

	async function loadStatus() {
		console.log('[IntelligenceCenter] Syncing status...');
		try {
			status = await invoke<DatabaseStatus>('get_database_status');
			diagnostics = await invoke<HardwareDiagnostics>('get_hardware_diagnostics');
			console.log('[IntelligenceCenter] Diagnostics loaded:', diagnostics);
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
			analysisStatus = 'Scanning for pending audits...';
			const count = await invoke<number>('analyze_all_records');
			addToast({
				type: 'info',
				message: `Neural Indexing initiated for ${count} records.`,
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
				'CRITICAL ACTION: This will purge all existing intelligence, OCR results, and forensic summaries and rerun the entire extraction pipeline. Proceed?'
			)
		)
			return;

		try {
			if (onAnalyze) onAnalyze();
			analysisActive = true;
			analysisProgress = 0;
			analysisStatus = 'Resetting Archive Intelligence...';
			const count = await invoke<number>('reprocess_all_records');
			addToast({
				type: 'info',
				message: `Neural Re-Audit initiated for ${count} records.`,
				duration: 5000
			});
		} catch (e) {
			addToast({ type: 'error', message: `Re-Audit failed: ${e}`, duration: 5000 });
			analysisActive = false;
		}
	}

	onMount(() => {
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
		}>('analysis-progress', (event) => {
			const { current, total, status } = event.payload;

			if (status === 'completed' || status === 'batch-complete') {
				analysisActive = false;
				analysisStatus = 'Intelligence Standby';
				loadStatus();
			} else if (status === 'synthesizing' || status === 'synthesizing-start') {
				analysisActive = true;
				const curToken = event.payload.token_index ?? 0;
				const totToken = event.payload.token_limit ?? 2048;
				analysisStatus = `Neural Synthesis: Auditing Artifact...`;
				analysisProgress = (curToken / totToken) * 100;
			} else {
				analysisActive = true;
				const cur = current ?? 0;
				const tot = total ?? 0;
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
			<!-- Hardware Diagnostics -->
			<section class="center-card diagnostics">
				<header>
					<Cpu size={18} />
					<h3>Hardware Diagnostics</h3>
				</header>
				{#if diagnostics}
					<div class="diag-metrics">
						<div class="metric">
							<span>Processor</span>
							<strong>{diagnostics.cpu_brand || 'Generic CPU'}</strong>
						</div>
						<div class="metric">
							<span>Memory Pool</span>
							<strong>{diagnostics.total_memory_gb} GB Total</strong>
						</div>
						<div class="metric">
							<span>Acceleration</span>
							<strong
								class={diagnostics.gpu_acceleration_available ? 'text-success' : 'text-warning'}
							>
								{diagnostics.gpu_acceleration_available
									? 'GPU Active (Metal/CUDA)'
									: 'CPU Only (Fallback)'}
							</strong>
						</div>
						<div class="metric">
							<span>Intelligence Tier</span>
							<strong class="tier-badge {diagnostics.recommended_tier}">
								{diagnostics.recommended_tier}
							</strong>
						</div>
					</div>
				{:else}
					<div class="loading-state">Probing hardware...</div>
				{/if}
			</section>

			<!-- Model Management -->
			<section class="center-card models">
				<header>
					<Database size={18} />
					<div class="header-content">
						<h3>Cognitive Models</h3>
						{#if models.some((m) => m.status === 'missing')}
							<button class="text-btn" onclick={provisionAll} disabled={!!busyModelId}>
								<Download size={14} /> Provision All Missing
							</button>
						{/if}
					</div>
				</header>
				<div class="model-list">
					{#each models as model (model.id)}
						<div class="model-item" class:busy={busyModelId === model.id}>
							<div class="model-info">
								<span class="m-type">{model.type}</span>
								<span class="m-name">{model.name}</span>
								{#if model.status === 'downloading'}
									<div class="progress-container">
										<div class="progress-bar" style="width: {model.progress}%"></div>
										<div class="m-stats">
											<span class="m-size">{model.progress.toFixed(1)}% of {model.size}</span>
											<span class="m-eta">
												{#if model.speedMbps !== null && model.speedMbps > 0}
													{model.speedMbps.toFixed(2)} MB/s
												{:else}
													...
												{/if}
												{#if model.etaSeconds !== null}
													• ETA: {model.etaSeconds}s
												{/if}
											</span>
										</div>
									</div>
								{:else}
									<span class="m-size">{model.size} • {model.status}</span>
								{/if}
							</div>
							<div class="model-actions">
								{#if busyModelId === model.id}
									<Loader2 class="spin" size={18} />
								{:else if model.status === 'ready'}
									<CheckCircle2 class="text-success" size={18} />
								{:else}
									<button class="icon-btn" onclick={() => downloadModel(model.id)}>
										<Download size={18} />
									</button>
								{/if}
							</div>
						</div>
					{/each}
				</div>
			</section>

			<!-- Vector Database Status -->
			<section class="center-card vector">
				<header>
					<HardDrive size={18} />
					<div class="header-content">
						<h3>Vector Index Analytics</h3>
						{#if analysisActive}
							<div class="analysis-progress">
								<span class="status-text">{analysisStatus}</span>
								<div class="progress-bar-bg">
									<div class="progress-bar-fill" style="width: {analysisProgress}%"></div>
								</div>
							</div>
						{:else}
							<div class="model-meta">
								<span>BGE v1.5 (384d)</span>
								<div class="actions">
									<button class="text-btn" onclick={reindexAll}>
										<Zap size={14} /> Audit Pending
									</button>
									<button class="text-btn danger" onclick={forceReprocessAll}>
										<RefreshCw size={14} /> Force Re-Audit
									</button>
								</div>
							</div>
						{/if}
					</div>
				</header>
				{#if status}
					<div class="diag-metrics">
						<div class="metric">
							<span>Indexed Chunks</span>
							<strong>{status.vector_chunks}</strong>
						</div>
						<div class="metric">
							<span>Entity Associations</span>
							<strong>{status.entity_count}</strong>
						</div>
						<div class="metric">
							<span>Storage Overhead</span>
							<strong>{(status.artifact_bytes / 1024 / 1024).toFixed(1)} MB</strong>
						</div>
						<div class="metric">
							<span>Search Engine</span>
							<strong>ONNX / Vector (BGE)</strong>
						</div>
						<div class="metric">
							<span>OCR Infrastructure</span>
							<strong>Native (Vision/Media)</strong>
						</div>
					</div>
				{:else}
					<div class="loading-state">Syncing index status...</div>
				{/if}
			</section>
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

	.center-card {
		background: var(--bg-surface);
		border: 1px solid var(--border-subtle);
		border-radius: var(--radius-lg);
		padding: 24px;
		display: flex;
		flex-direction: column;
		gap: 20px;
	}

	.center-card header {
		display: flex;
		align-items: center;
		gap: 12px;
		color: var(--text-secondary);
	}

	.center-card h3 {
		margin: 0;
		font-size: 14px;
		text-transform: uppercase;
		letter-spacing: 0.1em;
		font-weight: 700;
		flex: 1;
	}

	.header-content {
		display: flex;
		align-items: center;
		gap: 12px;
		width: 100%;
	}

	.text-btn {
		background: none;
		border: none;
		color: var(--accent-primary);
		font-size: 11px;
		font-weight: 700;
		text-transform: uppercase;
		display: flex;
		align-items: center;
		gap: 6px;
		cursor: pointer;
		padding: 4px 8px;
		border-radius: 4px;
		transition: background 0.2s;
	}

	.text-btn:hover {
		background: rgba(231, 196, 107, 0.1);
	}

	.text-btn:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	.diag-metrics {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: 20px;
	}

	.metric {
		display: flex;
		flex-direction: column;
		gap: 4px;
	}

	.metric span {
		font-size: 11px;
		color: var(--text-tertiary);
		text-transform: uppercase;
	}

	.metric strong {
		font-size: 15px;
		color: var(--text-primary);
	}

	.text-success {
		color: var(--accent-success) !important;
	}
	.text-warning {
		color: #f3c46b !important;
	}

	.tier-badge {
		display: inline-block;
		padding: 2px 8px;
		border-radius: 4px;
		background: rgba(231, 196, 107, 0.1);
		color: var(--accent-primary);
	}

	.model-list {
		display: flex;
		flex-direction: column;
		gap: 12px;
	}

	.model-item {
		display: flex;
		justify-content: space-between;
		align-items: center;
		padding: 16px;
		background: rgba(255, 255, 255, 0.02);
		border: 1px solid var(--border-subtle);
		border-radius: var(--radius-md);
		transition: var(--transition-fast);
	}

	.model-item.busy {
		border-color: var(--accent-primary);
		background: rgba(231, 196, 107, 0.05);
	}

	.progress-container {
		margin-top: 8px;
		width: 200px;
		height: 4px;
		background: rgba(255, 255, 255, 0.05);
		border-radius: 2px;
		position: relative;
		overflow: hidden;
	}

	.progress-bar {
		height: 100%;
		background: var(--accent-primary);
		box-shadow: 0 0 8px var(--accent-primary);
		transition: width 0.2s ease;
	}

	.model-info {
		display: flex;
		flex-direction: column;
		gap: 2px;
	}

	.m-type {
		font-size: 10px;
		text-transform: uppercase;
		color: var(--text-tertiary);
	}
	.m-name {
		font-size: 14px;
		font-weight: 600;
		color: var(--text-primary);
	}
	.m-size {
		font-size: 12px;
		color: var(--text-secondary);
	}

	.m-stats {
		display: flex;
		justify-content: space-between;
		margin-top: 4px;
		font-size: 11px;
	}

	.m-eta {
		color: var(--text-tertiary);
		font-family: var(--font-mono);
	}

	.icon-btn {
		width: 32px;
		height: 32px;
		display: flex;
		align-items: center;
		justify-content: center;
		border-radius: 50%;
		color: var(--text-secondary);
		transition: var(--transition-fast);
	}

	.icon-btn:hover {
		background: var(--bg-surface-elevated);
		color: var(--accent-primary);
	}

	.loading-state {
		padding: 20px;
		text-align: center;
		color: var(--text-tertiary);
		font-style: italic;
	}

	.analysis-progress {
		display: flex;
		flex-direction: column;
		gap: 4px;
		flex: 1;
		align-items: flex-end;
	}

	.status-text {
		font-size: 11px;
		color: var(--text-secondary);
	}

	.analysis-progress .progress-bar-bg {
		width: 100%;
		height: 4px;
		background: rgba(255, 255, 255, 0.05);
		border-radius: 2px;
		overflow: hidden;
	}

	.analysis-progress .progress-bar-fill {
		height: 100%;
		background: var(--accent-primary);
		transition: width 0.2s ease;
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
