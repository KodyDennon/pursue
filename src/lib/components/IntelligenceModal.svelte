<script lang="ts">
	import { onMount } from 'svelte';
	import { AlertCircle } from 'lucide-svelte';
	import { synthesisStore } from '$lib/stores/synthesisStore.svelte';

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

{#if isOpen}
	<div class="modal-overlay">
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
					<span>Neural inference utilizes Apple Neural Engine or local GPU. Keep application active.</span>
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
</style>
