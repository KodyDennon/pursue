<script lang="ts">
	import { Loader2, CheckCircle2, AlertCircle } from 'lucide-svelte';
	import { formatBytes } from '$lib/utils';
	import type { BulkDownloadReport } from '$lib/types';
	import { downloadStore } from '$lib/stores/downloadStore.svelte';

	let { report } = $props<{
		report: BulkDownloadReport;
	}>();
</script>

<div class="asset-list custom-scrollbar">
	{#each report.items as item (item.id)}
		<div class="asset-item {item.status}">
			<div class="asset-icon">
				{#if item.status === 'completed'}
					<CheckCircle2 size={14} class="text-success" />
				{:else if item.status === 'failed'}
					<AlertCircle size={14} class="text-error" />
				{:else if item.status === 'downloading'}
					<Loader2 size={14} class="spin text-accent" />
				{:else}
					<div class="dot"></div>
				{/if}
			</div>
			<div class="asset-details">
				<span class="asset-title">{item.title}</span>
				<span class="asset-meta">
					{#if item.status === 'completed'}
						{formatBytes(item.bytes_downloaded)} • Verified
					{:else if item.status === 'failed'}
						{item.error_class ? `${item.error_class}: ` : 'Error: '}{item.error ||
							'Unknown failure'}
					{:else if item.status === 'downloading' && downloadStore.itemProgress[item.id]}
						{formatBytes(downloadStore.itemProgress[item.id].bytes)}
						{#if downloadStore.itemProgress[item.id].total}
							/ {formatBytes(downloadStore.itemProgress[item.id].total)}
						{/if}
					{:else}
						{item.status}...
					{/if}
				</span>
			</div>
		</div>
	{/each}
</div>

<style>
	.asset-list {
		flex: 1;
		overflow-y: auto;
		display: flex;
		flex-direction: column;
		gap: var(--space-xs);
		padding-right: var(--space-md);
	}

	.asset-item {
		display: flex;
		gap: var(--space-xl);
		padding: var(--space-lg);
		border-radius: var(--radius-sm);
		background: rgba(255, 255, 255, 0.02);
		border: 1px solid transparent;
		transition: var(--transition-fast);
	}

	.asset-item:hover {
		background: rgba(255, 255, 255, 0.04);
	}

	.asset-icon {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 16px;
	}

	.dot {
		width: 4px;
		height: 4px;
		border-radius: 50%;
		background: var(--text-tertiary);
	}

	.asset-details {
		display: flex;
		flex-direction: column;
		gap: var(--space-2xs);
	}

	.asset-title {
		font-size: var(--text-md);
		color: var(--text-primary);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
		max-width: 400px;
	}

	.asset-meta {
		font-size: var(--text-sm);
		color: var(--text-tertiary);
	}

	.text-success {
		color: var(--color-accent-success);
	}
	.text-accent {
		color: var(--accent-primary);
	}
	.text-error {
		color: var(--color-accent-danger) !important;
	}

	:global(.spin) {
		animation: spin 1s linear infinite;
	}
	@keyframes spin {
		from {
			transform: rotate(0deg);
		}
		to {
			transform: rotate(360deg);
		}
	}
</style>
