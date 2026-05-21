<script lang="ts">
	import { onMount } from 'svelte';
	import { invoke } from '@tauri-apps/api/core';
	import { listen, type UnlistenFn } from '@tauri-apps/api/event';
	import Logo from '$lib/components/Logo.svelte';
	import { MODELS } from '$lib/models';

	import DiagnosticStep from './first_launch/DiagnosticStep.svelte';
	import SelectionStep from './first_launch/SelectionStep.svelte';
	import ProvisioningStep from './first_launch/ProvisioningStep.svelte';
	import ReadyStep from './first_launch/ReadyStep.svelte';

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
		// Check Diagnostics and Provisioning status
		(async () => {
			console.log('[FirstLaunch] Probing hardware diagnostics...');
			try {
				specs = await invoke<HardwareDiagnostics>('get_hardware_diagnostics');
				modelStatus = await invoke<Record<string, boolean>>('check_model_status');
				const runtimeProvisioned = await invoke<boolean>('check_neural_runtime_status');

				console.log('[FirstLaunch] Diagnostics:', specs);
				console.log('[FirstLaunch] Model Status:', modelStatus);
				console.log('[FirstLaunch] Runtime Status:', runtimeProvisioned);

				selectedTier = specs.recommended_tier === 'Elite' ? 'Elite' : 'Standard';

				const requiredIds = MODELS[selectedTier].map((m) => m.id);
				const allPresent = requiredIds.every((id) => modelStatus[id]) && runtimeProvisioned;

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
		let unlistenAnalysis: UnlistenFn;

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

		listen<{ status: string; progress?: number; msg?: string }>(
			'analysis-progress',
			(event) => {
				const payload = event.payload;
				if (payload.status === 'loading-model') {
					modelProgress = payload.progress ?? modelProgress;
					statusText = payload.msg || statusText;

					const rawOverall =
						totalModels > 0
							? Math.round(((modelsCompleted * 100 + modelProgress) / (totalModels * 100)) * 100)
							: 0;
					progressWidth = Math.min(99, rawOverall);
					overallProgress = progressWidth;
				}
			}
		).then((u) => (unlistenAnalysis = u));

		return () => {
			if (unlisten) unlisten();
			if (unlistenAnalysis) unlistenAnalysis();
		};
	});

	async function startProvisioning() {
		step = 'provisioning';
		const models = MODELS[selectedTier];
		// We add 1 for the Neural Vision Runtime
		totalModels = models.length + 1;
		modelsCompleted = 0;
		overallProgress = 0;
		let skipped = 0;

		// 1. Provision Cognitive Models
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
				statusText = `[${i + 1}/${totalModels}] ${model.name} — Already cached`;
				modelProgress = 100;
				speedMbps = null;
				etaSeconds = null;
				modelsCompleted = i + 1;
				overallProgress = Math.round((modelsCompleted / totalModels) * 100);
				skipped++;
				await new Promise((r) => setTimeout(r, 500));
				continue;
			}

			statusText = `[${i + 1}/${totalModels}] Downloading ${model.name}...`;
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
				statusText = `[${i + 1}/${totalModels}] ${model.name} downloaded`;
				await new Promise((r) => setTimeout(r, 300));
			} catch (e) {
				console.error(`Failed to download ${model.name}`, e);
				statusText = `[${i + 1}/${totalModels}] Error: ${model.name}. Retrying...`;
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
					statusText = `[${i + 1}/${totalModels}] ${model.name} downloaded`;
				} catch (e2) {
					console.error(`Critical failure for ${model.name}`, e2);
					statusText = `[${i + 1}/${totalModels}] ✗ Failed: ${model.name}`;
					modelsCompleted = i + 1;
					overallProgress = Math.round((modelsCompleted / totalModels) * 100);
					await new Promise((r) => setTimeout(r, 1500));
				}
			}
		}

		// 2. Provision Neural Vision Runtime (Python)
		currentModelIndex = totalModels;
		currentModelName = 'Neural Vision Runtime';
		modelProgress = 0;
		speedMbps = null;
		etaSeconds = null;

		const runtimeProvisioned = await invoke<boolean>('check_neural_runtime_status');
		if (runtimeProvisioned) {
			statusText = `[${totalModels}/${totalModels}] Neural Runtime — Verified`;
			modelProgress = 100;
			modelsCompleted = totalModels;
			overallProgress = 100;
			skipped++;
			await new Promise((r) => setTimeout(r, 500));
		} else {
			statusText = `[${totalModels}/${totalModels}] Provisioning Neural Runtime...`;
			try {
				await invoke('provision_neural_runtime');
				modelProgress = 100;
				modelsCompleted = totalModels;
				overallProgress = 100;
				statusText = `[${totalModels}/${totalModels}] Neural Runtime ready`;
				await new Promise((r) => setTimeout(r, 1000));
			} catch (e) {
				console.error('Failed to provision neural runtime', e);
				statusText = `[${totalModels}/${totalModels}] ✗ Runtime Provisioning Failed: ${e}`;
				modelsCompleted = totalModels;
				overallProgress = 100;
				await new Promise((r) => setTimeout(r, 3000));
			}
		}

		step = 'ready';
		if (skipped === totalModels) {
			statusText = 'Intelligence OS already provisioned.';
		} else {
			statusText = `Intelligence OS initialized. (${skipped} cached, ${totalModels - skipped} provisioned)`;
		}
		setTimeout(onComplete, 1500);
	}
</script>

<div class="provision-screen">
	<div class="provision-card glass-panel" class:selection={step === 'selection'}>
		<Logo size={60} class="hero-logo" />
		<div class="brand-hero">PURSUE</div>

		{#if step === 'diagnostic'}
			<DiagnosticStep {statusText} />
		{:else if step === 'selection'}
			<SelectionStep
				{specs}
				{selectedTier}
				onSelectTier={(tier) => (selectedTier = tier)}
				onStartProvisioning={startProvisioning}
			/>
		{:else if step === 'provisioning'}
			<ProvisioningStep
				{statusText}
				{progressWidth}
				{currentModelName}
				{overallProgress}
				{speedMbps}
				{etaSeconds}
				{currentModelIndex}
				{totalModels}
			/>
		{:else if step === 'ready'}
			<ReadyStep {statusText} />
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

	.provision-card.selection {
		width: 720px;
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
</style>
