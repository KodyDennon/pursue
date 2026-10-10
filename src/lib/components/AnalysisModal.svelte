<script lang="ts">
	/* eslint-disable no-useless-assignment -- bindable props propagate modal progress to the page shell */
	import { onMount } from 'svelte';
	import { Layers, X, AlertCircle } from 'lucide-svelte';
	import { analysisStore } from '$lib/stores/analysisStore.svelte';
	import Modal from './Modal.svelte';

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

<Modal bind:isOpen>
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
					>Ingestion and OCR are hardware intensive. Do not close the application during active
					processing.</span
				>
			</div>
		</footer>
	</div>
</Modal>

<style>
	.analysis-panel {
		width: min(960px, 100%);
		height: min(680px, calc(100vh - 48px));
		max-height: calc(100vh - 48px);
		min-height: 0;
		display: flex;
		flex-direction: column;
		overflow: hidden;
	}

	.panel-header {
		flex-shrink: 0;
		padding: 16px 24px;
		display: flex;
		justify-content: space-between;
		align-items: center;
		gap: var(--space-3xl);
		border-bottom: 1px solid var(--color-border-subtle);
	}

	.brand {
		display: flex;
		gap: var(--space-3xl);
		align-items: center;
		min-width: 0;
	}

	.brand h2 {
		margin: 0;
		font-size: var(--text-2xl);
		font-weight: 600;
		letter-spacing: 0.02em;
	}
	.brand p {
		margin: 2px 0 0 0;
		font-size: var(--text-base);
		color: var(--color-text-secondary);
	}

	.close-btn {
		background: none;
		border: none;
		color: var(--color-text-tertiary);
		cursor: pointer;
		padding: var(--space-sm);
		border-radius: 50%;
		transition: all 0.2s;
	}

	.close-btn:hover {
		background: rgba(255, 255, 255, 0.05);
		color: #fff;
	}

	.panel-body {
		flex: 1 1 auto;
		min-height: 0;
		padding: 20px 24px;
		overflow: hidden;
	}

	.overhaul-grid {
		display: grid;
		grid-template-columns: minmax(240px, 300px) minmax(0, 1fr);
		gap: 20px;
		height: 100%;
		min-height: 0;
	}

	.panel-footer {
		flex-shrink: 0;
		padding: 12px 24px;
		background: rgba(0, 0, 0, 0.2);
		border-top: 1px solid var(--color-border-subtle);
	}

	.notice {
		display: flex;
		align-items: flex-start;
		gap: var(--space-lg);
		color: var(--color-text-tertiary);
		font-size: var(--text-sm);
		line-height: 1.4;
	}

	.notice span {
		min-width: 0;
	}

	:global(.accent-icon) {
		color: var(--color-accent-primary);
	}
</style>
