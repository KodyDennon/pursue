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
				<span
					>Neural inference utilizes Apple Neural Engine or local GPU. Keep application active.</span
				>
			</div>
		</footer>
	</div>
</Modal>

<style>
	.synthesis-panel {
		width: 100%;
		max-width: 960px;
		height: 100%;
		max-height: 620px;
		display: flex;
		flex-direction: column;
		overflow: hidden;
	}

	.panel-body {
		flex: 1;
		padding: var(--space-6xl);
		overflow: hidden;
	}

	.overhaul-grid {
		display: grid;
		grid-template-columns: 320px 1fr;
		gap: var(--space-6xl);
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
		gap: var(--space-lg);
		color: var(--text-tertiary);
		font-size: var(--text-sm);
	}
</style>
