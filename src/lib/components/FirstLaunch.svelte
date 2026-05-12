<script lang="ts">
	import { onMount } from 'svelte';
	import { invoke } from '@tauri-apps/api/core';
	import { listen, type UnlistenFn } from '@tauri-apps/api/event';
	import Logo from '$lib/components/Logo.svelte';
	import { MODELS } from '$lib/models';
	import {
		Cpu,
		Brain,
		CheckCircle,
		ChevronRight,
		Loader2
	} from 'lucide-svelte';

	let { onComplete } = $props<{ onComplete: () => void }>();

	interface HardwareDiagnostics {
		cpu_brand: string;
		total_memory_gb: number;
		recommended_tier: 'Standard' | 'Elite';
	}

	interface ModelProgress {
		total_bytes?: number;
		bytes_downloaded: number;
		status: string;
		speed_mbps?: number;
		eta_seconds?: number;
	}

	let step = $state<'diagnostic' | 'selection' | 'provisioning' | 'ready'>('diagnostic');
	let statusText = $state('Analyzing hardware environment...');
	let modelProgress = $state(0);
	let overallProgress = $state(0);
	let progressWidth = $state(0);
	let currentModelIndex = $state(0);
	let totalModels = $state(0);
	let modelsCompleted = $state(0);
	let speedMbps = $state<number | null>(null);
	let etaSeconds = $state<number | null>(null);
	let specs = $state<HardwareDiagnostics | null>(null);
	let modelStatus = $state<Record<string, boolean>>({});
	let selectedTier = $state<'Standard' | 'Elite'>('Standard');
	let currentModelName = $state('');

	onMount(() => {
		console.log('[FirstLaunch] Component mounted.');
		// 1. Check Diagnostics and Provisioning status
		(async () => {
			console.log('[FirstLaunch] Probing hardware diagnostics...');
			try {
				specs = await invoke<HardwareDiagnostics>('get_hardware_diagnostics');
				modelStatus = await invoke<Record<string, boolean>>('check_model_status');

				console.log('[FirstLaunch] Diagnostics:', specs);
				console.log('[FirstLaunch] Model Status:', modelStatus);

				selectedTier = specs.recommended_tier === 'Elite' ? 'Elite' : 'Standard';

				const requiredIds = MODELS[selectedTier].map((m) => m.id);
				const allPresent = requiredIds.every((id) => modelStatus[id]);

				console.log('[FirstLaunch] Tier:', selectedTier, 'All present:', allPresent);

				if (allPresent) {
					step = 'ready';
					statusText = 'Intelligence OS already provisioned.';
					console.log('[FirstLaunch] Already provisioned, completing...');
					setTimeout(onComplete, 500);
					return;
				}

				step = 'selection';
				statusText = 'Environment scan complete.';
			} catch (e) {
				console.error('[FirstLaunch] Initialization probe failed', e);
				statusText = 'Hardware probe failed. Using standard profile.';
				step = 'selection';
			}
		})();

		// Listen for progress
		let unlisten: UnlistenFn;
		listen<ModelProgress>('model-progress', (event) => {
			const payload = event.payload;

			// Only update progress from byte-level download events
			if (payload.total_bytes && payload.total_bytes > 0) {
				modelProgress = Math.round((payload.bytes_downloaded / payload.total_bytes) * 100);
			}

			// Compute overall: completed models + fraction of current model
			const rawOverall =
				totalModels > 0
					? Math.round(((modelsCompleted * 100 + modelProgress) / (totalModels * 100)) * 100)
					: 0;
			progressWidth = Math.min(99, rawOverall); // Never hit 100 until the loop says so
			overallProgress = progressWidth;

			// Speed/ETA from active downloads only
			if (payload.status === 'downloading') {
				speedMbps = payload.speed_mbps !== undefined ? payload.speed_mbps : null;
				etaSeconds = payload.eta_seconds !== undefined ? payload.eta_seconds : null;
			}
		}).then((u) => (unlisten = u));

		return () => {
			if (unlisten) unlisten();
		};
	});

	async function startProvisioning() {
		step = 'provisioning';
		const models = MODELS[selectedTier];
		totalModels = models.length;
		modelsCompleted = 0;
		overallProgress = 0;
		let skipped = 0;

		for (let i = 0; i < models.length; i++) {
			const model = models[i];
			currentModelIndex = i + 1;
			currentModelName = model.name;
			modelProgress = 0;
			speedMbps = null;
			etaSeconds = null;

			// Re-check status
			const currentStatus = await invoke<Record<string, boolean>>('check_model_status');
			if (currentStatus[model.id]) {
				statusText = `[${i + 1}/${models.length}] ${model.name} — Already cached`;
				modelProgress = 100;
				speedMbps = null;
				etaSeconds = null;
				modelsCompleted = i + 1;
				overallProgress = Math.round((modelsCompleted / totalModels) * 100);
				skipped++;
				await new Promise((r) => setTimeout(r, 500));
				continue;
			}

			statusText = `[${i + 1}/${models.length}] Downloading ${model.name}...`;
			modelProgress = 0;

			try {
				await invoke('provision_model', {
					id: model.id,
					url: model.url,
					name: model.filename
				});
				modelProgress = 100;
				speedMbps = null;
				etaSeconds = null;
				modelsCompleted = i + 1;
				overallProgress = Math.round((modelsCompleted / totalModels) * 100);
				statusText = `[${i + 1}/${models.length}] ${model.name} downloaded`;
				await new Promise((r) => setTimeout(r, 300));
			} catch (e) {
				console.error(`Failed to download ${model.name}`, e);
				statusText = `[${i + 1}/${models.length}] Error: ${model.name}. Retrying...`;
				await new Promise((r) => setTimeout(r, 2000));

				try {
					modelProgress = 0;
					await invoke('provision_model', {
						id: model.id,
						url: model.url,
						name: model.filename
					});
					modelProgress = 100;
					speedMbps = null;
					etaSeconds = null;
					modelsCompleted = i + 1;
					overallProgress = Math.round((modelsCompleted / totalModels) * 100);
					statusText = `[${i + 1}/${models.length}] ${model.name} downloaded`;
				} catch (e2) {
					console.error(`Critical failure for ${model.name}`, e2);
					statusText = `[${i + 1}/${models.length}] ✗ Failed: ${model.name}`;
					modelsCompleted = i + 1;
					overallProgress = Math.round((modelsCompleted / totalModels) * 100);
					await new Promise((r) => setTimeout(r, 1500));
				}
			}
		}

		step = 'ready';
		if (skipped === models.length) {
			statusText = 'Intelligence OS already provisioned.';
		} else {
			statusText = `Intelligence OS initialized. (${skipped} cached, ${models.length - skipped} downloaded)`;
		}
		setTimeout(onComplete, 1500);
	}
</script>

<div class="provision-screen">
	<div class="provision-card glass-panel" class:selection={step === 'selection'}>
		<Logo size={60} class="hero-logo" />
		<div class="brand-hero">PURSUE</div>

		{#if step === 'diagnostic'}
			<h2>Black-Ops Initialization</h2>
			<p class="status-mono mono">{statusText}</p>
			<div class="diagnostic-loader">
				<Loader2 size={24} class="spin" />
			</div>
		{:else if step === 'selection'}
			<div class="selection-view">
				<h2>Intelligence Tier Selection</h2>
				<p>
					Recommended based on your <strong>{specs?.cpu_brand || 'Processor'}</strong> and
					<strong>{specs?.total_memory_gb || '??'}GB RAM</strong>.
				</p>

				<div class="tier-options">
					<button
						class="tier-card"
						class:active={selectedTier === 'Standard'}
						class:recommended={specs?.recommended_tier === 'Standard'}
						onclick={() => (selectedTier = 'Standard')}
					>
						<div class="tier-head">
							<Cpu size={24} />
							<div class="t-title">Standard Intel</div>
						</div>
						<p>
							Gemma 4 E2B + BGE. Optimized effective parameter architecture for workstation
							performance.
						</p>
						<div class="tier-meta">~3.2 GB Storage</div>
					</button>

					<button
						class="tier-card"
						class:active={selectedTier === 'Elite'}
						class:recommended={specs?.recommended_tier === 'Elite'}
						onclick={() => (selectedTier = 'Elite')}
					>
						<div class="tier-head">
							<Brain size={24} />
							<div class="t-title">Elite Intel</div>
						</div>
						<p>Gemma 4 E4B + BGE. Advanced reasoning with native multimodal capabilities.</p>
						<div class="tier-meta">~5.0 GB Storage</div>
					</button>
				</div>

				<button class="provision-btn" onclick={startProvisioning}>
					Initialize Neural OS <ChevronRight size={18} />
				</button>
			</div>
		{:else if step === 'provisioning'}
			<h2>Provisioning Intelligence Engine</h2>
			<p class="status-mono mono">{statusText}</p>

			<div class="progress-bar-wrap">
				<div class="progress-fill" style="width: {progressWidth}%"></div>
			</div>

			<div class="sys-reqs">
				<span>{currentModelName}</span>
				<span>{overallProgress}%</span>
			</div>

			<div class="dl-stats">
				{#if speedMbps !== null && speedMbps > 0}
					<span>{speedMbps.toFixed(2)} MB/s</span>
				{:else}
					<span>...</span>
				{/if}
				{#if etaSeconds !== null && etaSeconds > 0}
					<span>ETA: {Math.floor(etaSeconds / 60)}m {etaSeconds % 60}s</span>
				{/if}
			</div>

			<div class="step-counter">
				<span>Model {currentModelIndex} of {totalModels}</span>
			</div>
		{:else if step === 'ready'}
			<h2>Systems Ready</h2>
			<p class="status-mono mono">{statusText}</p>
			<div class="ready-check">
				<CheckCircle size={48} class="accent-success" />
			</div>
		{/if}
	</div>
</div>

<style>
	.provision-screen {
		position: fixed;
		inset: 0;
		background: #050608;
		display: flex;
		align-items: center;
		justify-content: center;
		z-index: 9999;
	}

	.provision-card {
		width: 480px;
		padding: 40px;
		display: flex;
		flex-direction: column;
		align-items: center;
		text-align: center;
		box-shadow: 0 0 50px rgba(0, 0, 0, 0.5);
	}

	:global(.hero-logo) {
		margin-bottom: 32px;
		filter: drop-shadow(0 0 20px rgba(231, 196, 107, 0.3));
	}

	.brand-hero {
		font-family: var(--font-display);
		font-weight: 700;
		font-size: 36px;
		letter-spacing: 0.25em;
		color: var(--accent-primary);
		margin-bottom: 24px;
		text-shadow: 0 0 10px rgba(231, 196, 107, 0.2);
	}

	h2 {
		font-size: 20px;
		margin-bottom: 8px;
		color: var(--text-primary);
		letter-spacing: 0.05em;
	}

	p {
		font-size: 14px;
		color: var(--text-secondary);
		margin-bottom: 32px;
	}

	.status-mono {
		font-size: 11px;
		color: var(--accent-success);
		margin-bottom: 24px;
		text-transform: uppercase;
		letter-spacing: 0.1em;
	}

	.diagnostic-loader {
		padding: 20px;
		color: var(--accent-primary);
	}

	.progress-bar-wrap {
		width: 100%;
		height: 6px;
		background: rgba(255, 255, 255, 0.05);
		border-radius: 3px;
		overflow: hidden;
		margin-bottom: 16px;
		border: 1px solid rgba(255, 255, 255, 0.02);
	}

	.progress-fill {
		height: 100%;
		background: linear-gradient(90deg, var(--accent-primary), #f5d547);
		box-shadow: 0 0 15px var(--accent-primary);
		transition: width 0.2s cubic-bezier(0.25, 0.46, 0.45, 0.94);
	}

	.sys-reqs {
		display: flex;
		justify-content: space-between;
		width: 100%;
		font-size: 10px;
		color: var(--text-tertiary);
		text-transform: uppercase;
		letter-spacing: 0.1em;
		font-weight: 700;
	}

	.dl-stats {
		display: flex;
		justify-content: space-between;
		width: 100%;
		font-size: 10px;
		color: var(--text-tertiary);
		margin-top: 6px;
		font-family: var(--font-mono);
	}

	.step-counter {
		margin-top: 16px;
		font-size: 9px;
		color: var(--text-tertiary);
		text-transform: uppercase;
		letter-spacing: 0.15em;
		font-weight: 600;
	}

	.tier-options {
		display: flex;
		gap: 16px;
		margin-bottom: 40px;
		width: 100%;
	}

	.tier-card {
		flex: 1;
		background: rgba(255, 255, 255, 0.01);
		border: 1px solid var(--border-subtle);
		border-radius: 12px;
		padding: 24px;
		text-align: left;
		cursor: pointer;
		transition: all 0.3s;
		display: flex;
		flex-direction: column;
		gap: 12px;
	}

	.tier-card p {
		font-size: 12px;
		margin: 0;
		line-height: 1.5;
		color: var(--text-tertiary);
	}

	.tier-card.active {
		border-color: var(--accent-primary);
		background: rgba(231, 196, 107, 0.05);
		box-shadow: 0 0 20px rgba(231, 196, 107, 0.1);
	}

	.tier-card.recommended {
		position: relative;
	}
	.tier-card.recommended::before {
		content: 'RECOMMENDED';
		position: absolute;
		top: -10px;
		right: 12px;
		font-size: 8px;
		background: var(--accent-primary);
		color: #000;
		padding: 2px 6px;
		border-radius: 4px;
		font-weight: 800;
	}

	.tier-head {
		display: flex;
		align-items: center;
		gap: 12px;
		color: var(--accent-primary);
	}

	.t-title {
		font-weight: 700;
		font-size: 15px;
		color: var(--text-primary);
	}

	.tier-meta {
		font-size: 10px;
		color: var(--text-tertiary);
		text-transform: uppercase;
		margin-top: auto;
	}

	.provision-btn {
		width: 100%;
		background: var(--accent-primary);
		color: #000;
		border: none;
		border-radius: 8px;
		padding: 16px;
		font-weight: 800;
		font-size: 15px;
		display: flex;
		align-items: center;
		justify-content: center;
		gap: 12px;
		cursor: pointer;
		transition: all 0.3s;
		text-transform: uppercase;
		letter-spacing: 0.1em;
	}

	.provision-btn:hover {
		filter: brightness(1.1);
		transform: translateY(-2px);
	}

	.provision-card.selection {
		width: 720px;
	}

	.ready-check {
		margin-top: 20px;
		color: var(--accent-success);
		animation: scale-in 0.5s cubic-bezier(0.175, 0.885, 0.32, 1.275);
	}

	@keyframes scale-in {
		from {
			transform: scale(0);
			opacity: 0;
		}
		to {
			transform: scale(1);
			opacity: 1;
		}
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
