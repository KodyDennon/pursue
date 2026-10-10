<script lang="ts">
	/* eslint-disable no-useless-assignment -- bindable props propagate modal progress to the page shell */
	import { onMount } from 'svelte';
	import { AlertCircle } from 'lucide-svelte';
	import { synthesisStore } from '$lib/stores/synthesisStore.svelte';
	import Modal from './Modal.svelte';

	import IntelligenceModalHeader from './intelligence_modal/IntelligenceModalHeader.svelte';
	import SynthesisTelemetry from './intelligence_modal/SynthesisTelemetry.svelte';
	import CognitiveStream from './intelligence_modal/CognitiveStream.svelte';

	let {
		isOpen = $bindable(false),
		isBusy = $bindable(false),
		onComplete
	} = $props<{
		isOpen: boolean;
		isBusy?: boolean;
		onComplete?: () => void;
	}>();

	$effect(() => {
		isBusy = synthesisStore.busy;
	});

	onMount(() => {
		synthesisStore.init(isOpen, (open) => (isOpen = open), onComplete);
		return () => synthesisStore.destroy();
	});

	function close() {
		isOpen = false;
	}
</script>

<Modal bind:isOpen>
	<div class="synthesis-panel glass-panel">
		<IntelligenceModalHeader {close} />

		<div class="panel-body">
			<div class="overhaul-grid">
				<SynthesisTelemetry
					status={synthesisStore.status}
					busy={synthesisStore.busy}
					currentRecordId={synthesisStore.currentRecordId}
					currentBatchIndex={synthesisStore.currentBatchIndex}
					totalBatchCount={synthesisStore.totalBatchCount}
					modelDownloadProgress={synthesisStore.modelDownloadProgress}
					modelDownloadMsg={synthesisStore.modelDownloadMsg}
					neuralTelemetry={synthesisStore.neuralTelemetry}
					onDismiss={close}
				/>

				<CognitiveStream
					status={synthesisStore.status}
					thoughtText={synthesisStore.thoughtText}
					modelDownloadMsg={synthesisStore.modelDownloadMsg}
				/>
			</div>
		</div>

		<footer class="panel-footer">
			<div class="notice">
				<AlertCircle size={14} />
				<span>Keep PURSUE open until this batch finishes.</span>
			</div>
		</footer>
	</div>
</Modal>

<style>
	.synthesis-panel {
		width: min(960px, 100%);
		height: min(680px, calc(100vh - 48px));
		max-height: calc(100vh - 48px);
		min-height: 0;
		display: flex;
		flex-direction: column;
		overflow: hidden;
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
</style>
