<script lang="ts">
	import type { IntelligenceLog } from '$lib/types';

	let { intelLogs } = $props<{
		intelLogs: IntelligenceLog[];
	}>();
</script>

<div class="view-padding">
	<header class="section-head"><span class="prefix">NEURAL LOGSTACK</span></header>
	<div class="log-stack">
		{#each intelLogs as log (log.id)}
			<div class="log-entry-item">
				<header>{log.model_id} @ {new Date(log.created_at).toLocaleTimeString()}</header>
				<pre>{log.response_json}</pre>
			</div>
		{/each}
	</div>
</div>

<style>
	.view-padding {
		padding: var(--space-7xl);
	}

	.section-head {
		margin-bottom: var(--space-4xl);
		border-bottom: 1px solid rgba(255, 255, 255, 0.05);
		padding-bottom: var(--space-md);
	}

	.prefix {
		font-size: var(--text-2xs);
		font-weight: 900;
		letter-spacing: 0.15em;
		color: var(--color-text-tertiary);
	}

	.log-stack {
		display: flex;
		flex-direction: column;
		gap: var(--space-3xl);
	}

	.log-entry-item {
		background: rgba(255, 255, 255, 0.02);
		border: 1px solid var(--color-border-subtle);
		border-radius: var(--radius-base);
		overflow: hidden;
	}

	.log-entry-item header {
		background: rgba(255, 255, 255, 0.03);
		padding: 8px 16px;
		font-family: var(--font-mono);
		font-size: var(--text-xs);
		color: var(--color-accent-primary);
		border-bottom: 1px solid var(--color-border-subtle);
	}

	.log-entry-item pre {
		padding: var(--space-3xl);
		font-family: var(--font-mono);
		font-size: var(--text-sm);
		color: var(--color-text-secondary);
		white-space: pre-wrap;
		margin: 0;
	}
</style>
