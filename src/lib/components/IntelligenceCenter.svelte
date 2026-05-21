<script lang="ts">
	import { onMount } from 'svelte';
	import { Brain, Loader2 } from 'lucide-svelte';
	import { intelligenceStore } from '$lib/stores/intelligenceStore.svelte';

	import HardwareDiagnostics from './intelligence_center/HardwareDiagnostics.svelte';
	import ModelManager from './intelligence_center/ModelManager.svelte';
	import VectorAnalytics from './intelligence_center/VectorAnalytics.svelte';
	import CognitiveSynthesis from './intelligence_center/CognitiveSynthesis.svelte';

	let { onAnalyze, onSynthesize } = $props<{
		onAnalyze?: () => void;
		onSynthesize?: () => void;
	}>();

	onMount(() => {
		intelligenceStore.init();
		return () => intelligenceStore.destroy();
	});
</script>

{#if !intelligenceStore.status || !intelligenceStore.diagnostics}
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
			<HardwareDiagnostics diagnostics={intelligenceStore.diagnostics} />

			<ModelManager
				models={intelligenceStore.models}
				runtimeProvisioned={intelligenceStore.runtimeProvisioned}
				runtimeBusy={intelligenceStore.runtimeBusy}
				busyModelId={intelligenceStore.busyModelId}
				onProvisionRuntime={() => intelligenceStore.provisionRuntime()}
				onDownloadModel={(id) => intelligenceStore.downloadModel(id)}
				onProvisionAll={() => intelligenceStore.provisionAll()}
			/>

			<VectorAnalytics
				status={intelligenceStore.status}
				analysisActive={intelligenceStore.analysisActive}
				analysisStatus={intelligenceStore.analysisStatus}
				analysisProgress={intelligenceStore.analysisProgress}
				onReindexAll={() => intelligenceStore.reindexAll(onAnalyze)}
				onForceReprocessAll={() => intelligenceStore.forceReprocessAll(onAnalyze)}
			/>

			<CognitiveSynthesis
				status={intelligenceStore.status}
				analysisActive={intelligenceStore.analysisActive}
				analysisStatus={intelligenceStore.analysisStatus}
				analysisProgress={intelligenceStore.analysisProgress}
				onRunBatchSynthesis={() => intelligenceStore.runBatchSynthesis(onSynthesize)}
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

	:global(.accent-icon) {
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
