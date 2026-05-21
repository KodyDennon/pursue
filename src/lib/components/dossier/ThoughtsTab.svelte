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
		padding: 32px;
	}

	.section-head {
		margin-bottom: 20px;
		border-bottom: 1px solid rgba(255, 255, 255, 0.05);
		padding-bottom: 8px;
	}

	.prefix {
		font-size: 9px;
		font-weight: 900;
		letter-spacing: 0.15em;
		color: var(--text-tertiary);
	}

	.log-stack {
		display: flex;
		flex-direction: column;
		gap: 16px;
	}

	.log-entry-item {
		background: rgba(255, 255, 255, 0.02);
		border: 1px solid var(--border-subtle);
		border-radius: 8px;
		overflow: hidden;
	}

	.log-entry-item header {
		background: rgba(255, 255, 255, 0.03);
		padding: 8px 16px;
		font-family: var(--font-mono);
		font-size: 10px;
		color: var(--accent-primary);
		border-bottom: 1px solid var(--border-subtle);
	}

	.log-entry-item pre {
		padding: 16px;
		font-family: var(--font-mono);
		font-size: 11px;
		color: var(--text-secondary);
		white-space: pre-wrap;
		margin: 0;
	}
</style>
