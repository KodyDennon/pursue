<script lang="ts">
	import { formatBytes } from '$lib/utils';
	import { intelligenceStore } from '$lib/stores/intelligenceStore.svelte';
</script>

{#if intelligenceStore.status}
	<div class="stats-bar">
		<span class="stat">Total Records: <strong>{intelligenceStore.status.total_count}</strong></span>
		<span class="stat"
			>Vault Storage: <strong>{formatBytes(intelligenceStore.status.total_size)}</strong></span
		>
		<span class="stat">Database: <strong>Online</strong></span>
	</div>
{/if}

<style>
	.stats-bar {
		display: flex;
		align-items: center;
		gap: var(--space-5xl);
		padding: 8px 32px;
		background: rgba(0, 0, 0, 0.2);
		border-bottom: 1px solid var(--border-subtle);
		font-size: var(--text-sm);
		text-transform: uppercase;
		color: var(--text-secondary);
		letter-spacing: 0.05em;
	}

	.stats-bar strong {
		color: var(--text-primary);
		margin-left: var(--space-xs);
	}
</style>
