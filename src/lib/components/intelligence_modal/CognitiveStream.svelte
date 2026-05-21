<script lang="ts">
	import { Brain, Activity } from 'lucide-svelte';

	let { status, thoughtText, modelDownloadMsg } = $props<{
		status: string;
		thoughtText: string;
		modelDownloadMsg: string;
	}>();
</script>

<div class="stream-side">
	<div class="section-head">
		<Activity size={14} />
		<span>Cognitive Thought Block</span>
	</div>
	<div class="stream-viewport custom-scrollbar">
		{#if status === 'loading-model'}
			<div class="model-loading-fullscreen">
				<Brain size={48} class="accent-icon pulse-brain" />
				<h3>PROVISIONING NEURAL RUNTIME</h3>
				<p>{modelDownloadMsg || 'Mounting tensor files into hardware cache...'}</p>
			</div>
		{:else if thoughtText}
			<div class="neural-stream">
				{thoughtText}<span class="cursor">█</span>
			</div>
		{:else}
			<div class="empty-state">Thought stream will manifest here upon inference start.</div>
		{/if}
	</div>
</div>

<style>
	.stream-side {
		display: flex;
		flex-direction: column;
		gap: 10px;
		overflow: hidden;
		border-left: 1px solid var(--border-subtle);
		padding-left: 28px;
	}

	.section-head {
		display: flex;
		align-items: center;
		gap: 8px;
		color: var(--text-tertiary);
		font-size: 9px;
		font-weight: 800;
		letter-spacing: 0.15em;
		text-transform: uppercase;
	}

	.stream-viewport {
		flex: 1;
		background: rgba(0, 0, 0, 0.4);
		border: 1px solid var(--border-subtle);
		border-radius: var(--radius-md);
		padding: 20px;
		overflow-y: auto;
	}

	.model-loading-fullscreen {
		height: 100%;
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		text-align: center;
		gap: 16px;
	}

	.model-loading-fullscreen h3 {
		font-size: 13px;
		letter-spacing: 0.1em;
		color: var(--text-primary);
	}

	.model-loading-fullscreen p {
		font-size: 11px;
		font-family: var(--font-mono);
		color: var(--text-secondary);
		opacity: 0.8;
	}

	.neural-stream {
		font-family: var(--font-mono);
		font-size: 12px;
		line-height: 1.7;
		color: var(--accent-primary);
		white-space: pre-wrap;
		word-break: break-all;
		text-shadow: 0 0 5px rgba(231, 196, 107, 0.2);
	}

	.empty-state {
		height: 100%;
		display: flex;
		align-items: center;
		justify-content: center;
		color: var(--text-tertiary);
		opacity: 0.5;
		font-style: italic;
		font-size: 11px;
	}

	.cursor {
		display: inline-block;
		animation: blink 1s step-end infinite;
		color: var(--accent-primary);
	}

	@keyframes blink {
		from,
		to {
			opacity: 1;
		}
		50% {
			opacity: 0;
		}
	}

	:global(.pulse-brain) {
		animation: pulse-brain-anim 2.5s infinite ease-in-out;
		filter: drop-shadow(0 0 12px rgba(231, 196, 107, 0.5));
	}

	@keyframes pulse-brain-anim {
		0%,
		100% {
			transform: scale(1);
			opacity: 0.8;
		}
		50% {
			transform: scale(1.08);
			opacity: 1;
			filter: drop-shadow(0 0 20px rgba(231, 196, 107, 0.8));
		}
	}

	:global(.accent-icon) {
		color: var(--accent-primary);
	}
</style>
