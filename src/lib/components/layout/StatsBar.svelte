<script lang="ts">
	import type { DatabaseStatus } from '$lib/types';
	import { formatBytes } from '$lib/utils';

	let { databaseStatus } = $props<{ databaseStatus: DatabaseStatus | null }>();
</script>

{#if databaseStatus}
	<div class="stats-bar">
		<span class="stat">Total Records: <strong>{databaseStatus.total_count}</strong></span>
		<span class="stat"
			>Vault Storage: <strong>{formatBytes(databaseStatus.total_size)}</strong></span
		>
		<span class="stat">Database: <strong>Online</strong></span>
	</div>
{/if}

<style>
	.stats-bar {
		display: flex;
		align-items: center;
		gap: 24px;
		padding: 8px 32px;
		background: rgba(0, 0, 0, 0.2);
		border-bottom: 1px solid var(--border-subtle);
		font-size: 11px;
		text-transform: uppercase;
		color: var(--text-secondary);
		letter-spacing: 0.05em;
	}

	.stats-bar strong {
		color: var(--text-primary);
		margin-left: 4px;
	}
</style>
