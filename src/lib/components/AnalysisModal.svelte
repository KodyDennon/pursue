<script lang="ts">
	import { onMount } from 'svelte';
	import { Layers, X, AlertCircle } from 'lucide-svelte';
	import { analysisStore } from '$lib/stores/analysisStore.svelte';

	import NeuralTelemetry from './analysis_modal/NeuralTelemetry.svelte';
	import ThoughtStream from './analysis_modal/ThoughtStream.svelte';

	let {
		isOpen = $bindable(false),
		isBusy = $bindable(false),
		progress = $bindable(0),
		onComplete
	} = $props<{
		isOpen: boolean;
		isBusy?: boolean;
		progress?: number;
		onComplete?: () => void;
	}>();

	$effect(() => {
		isBusy = analysisStore.busy;
		progress = analysisStore.progress;
	});

	onMount(() => {
		analysisStore.init(isOpen, (open) => (isOpen = open), onComplete);
		return () => analysisStore.destroy();
	});

	function close() {
		isOpen = false;
	}
</script>

{#if isOpen}
	<div class="modal-overlay">
		<div class="analysis-panel glass-panel">
			<header class="panel-header glass-header">
				<div class="brand">
					<Layers size={24} class="accent-icon" />
					<div>
						<h2>Secure Ingestion & Foundation Audit</h2>
						<p>High-resolution OCR extraction and semantic vector mapping.</p>
					</div>
				</div>
				<button class="close-btn" onclick={close} aria-label="Close modal"><X size={20} /></button>
			</header>

			<div class="panel-body">
				<div class="overhaul-grid">
					<NeuralTelemetry
						status={analysisStore.status}
						processedCount={analysisStore.processedCount}
						totalCount={analysisStore.totalCount}
						progress={analysisStore.progress}
						currentRecordId={analysisStore.currentRecordId}
						busy={analysisStore.busy}
						ocrDownloadProgress={analysisStore.ocrDownloadProgress}
						ocrDownloadMsg={analysisStore.ocrDownloadMsg}
						onStartAnalysis={() => analysisStore.startAnalysis()}
					/>

					<ThoughtStream logs={analysisStore.logs} />
				</div>
			</div>

			<footer class="panel-footer">
				<div class="notice">
					<AlertCircle size={14} />
					<span
						>Ingestion and OCR are hardware intensive. Do not close the application during
						active processing.</span
					>
				</div>
			</footer>
		</div>
	</div>
{/if}

<style>
	.modal-overlay {
		position: fixed;
		inset: 0;
		z-index: 2000;
		background: rgba(0, 0, 0, 0.85);
		backdrop-filter: blur(10px);
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 40px;
	}

	.analysis-panel {
		width: 100%;
		max-width: 960px;
		height: 100%;
		max-height: 620px;
		display: flex;
		flex-direction: column;
		overflow: hidden;
	}

	.panel-header {
		padding: 20px 28px;
		display: flex;
		justify-content: space-between;
		align-items: center;
		border-bottom: 1px solid var(--border-subtle);
	}

	.brand {
		display: flex;
		gap: 16px;
		align-items: center;
	}

	.brand h2 {
		margin: 0;
		font-size: 18px;
		font-weight: 600;
		letter-spacing: 0.02em;
	}
	.brand p {
		margin: 2px 0 0 0;
		font-size: 12px;
		color: var(--text-secondary);
	}

	.close-btn {
		background: none;
		border: none;
		color: var(--text-tertiary);
		cursor: pointer;
		padding: 6px;
		border-radius: 50%;
		transition: all 0.2s;
	}

	.close-btn:hover {
		background: rgba(255, 255, 255, 0.05);
		color: #fff;
	}

	.panel-body {
		flex: 1;
		padding: 28px;
		overflow: hidden;
	}

	.overhaul-grid {
		display: grid;
		grid-template-columns: 320px 1fr;
		gap: 28px;
		height: 100%;
	}

	.panel-footer {
		padding: 16px 28px;
		background: rgba(0, 0, 0, 0.2);
		border-top: 1px solid var(--border-subtle);
	}

	.notice {
		display: flex;
		align-items: center;
		gap: 10px;
		color: var(--text-tertiary);
		font-size: 11px;
	}

	:global(.accent-icon) {
		color: var(--accent-primary);
	}
</style>
