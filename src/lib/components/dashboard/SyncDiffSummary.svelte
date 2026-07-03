<script lang="ts">
	import { X, FilePlus, FileDiff, FileMinus } from 'lucide-svelte';
	import type { SyncReport } from '$lib/types';

	let { report, onDismiss } = $props<{
		report: SyncReport;
		onDismiss: () => void;
	}>();

	const CHANGE_LIMIT = 8;
	let visibleDiffs = $derived(report.diffs.slice(0, CHANGE_LIMIT));
	let overflowCount = $derived(Math.max(0, report.diffs.length - CHANGE_LIMIT));
</script>

<div class="diff-summary glass-panel">
	<div class="diff-summary-header">
		<span class="diff-summary-title">Latest sync: what changed</span>
		<div class="diff-summary-counts">
			<span class="count added">+{report.added} added</span>
			<span class="count changed">~{report.changed} changed</span>
			<span class="count removed">-{report.removed} removed</span>
		</div>
		<button class="dismiss-btn" onclick={onDismiss} aria-label="Dismiss">
			<X size={14} />
		</button>
	</div>

	{#if visibleDiffs.length > 0}
		<ul class="diff-list">
			{#each visibleDiffs as diff (diff.stable_key)}
				<li class="diff-item {diff.change_type}">
					<span class="diff-icon">
						{#if diff.change_type === 'added'}
							<FilePlus size={12} />
						{:else if diff.change_type === 'removed'}
							<FileMinus size={12} />
						{:else}
							<FileDiff size={12} />
						{/if}
					</span>
					<span class="diff-title">{diff.title}</span>
				</li>
			{/each}
		</ul>
		{#if overflowCount > 0}
			<div class="diff-overflow">+{overflowCount} more</div>
		{/if}
	{/if}
</div>

<style>
	.diff-summary {
		display: flex;
		flex-direction: column;
		gap: var(--space-lg);
		padding: 14px 18px;
		margin-bottom: var(--space-3xl);
	}

	.diff-summary-header {
		display: flex;
		align-items: center;
		gap: var(--space-3xl);
	}

	.diff-summary-title {
		font-size: var(--text-base);
		font-weight: 700;
		text-transform: uppercase;
		letter-spacing: 0.05em;
		color: var(--color-text-secondary);
	}

	.diff-summary-counts {
		display: flex;
		gap: var(--space-xl);
		font-size: var(--text-base);
		font-weight: 600;
	}

	.count.added {
		color: var(--color-accent-success);
	}
	.count.changed {
		color: var(--color-accent-primary);
	}
	.count.removed {
		color: var(--color-accent-danger);
	}

	.dismiss-btn {
		margin-left: auto;
		display: flex;
		align-items: center;
		justify-content: center;
		background: transparent;
		border: none;
		color: var(--color-text-tertiary);
		cursor: pointer;
		padding: var(--space-xs);
		border-radius: var(--radius-sm);
	}

	.dismiss-btn:hover {
		color: var(--color-text-primary);
		background: rgba(255, 255, 255, 0.05);
	}

	.diff-list {
		list-style: none;
		margin: 0;
		padding: 0;
		display: flex;
		flex-direction: column;
		gap: var(--space-xs);
	}

	.diff-item {
		display: flex;
		align-items: center;
		gap: var(--space-md);
		font-size: var(--text-base);
		color: var(--color-text-secondary);
	}

	.diff-item.added .diff-icon {
		color: var(--color-accent-success);
	}
	.diff-item.changed .diff-icon {
		color: var(--color-accent-primary);
	}
	.diff-item.removed .diff-icon {
		color: var(--color-accent-danger);
	}

	.diff-title {
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.diff-overflow {
		font-size: var(--text-sm);
		color: var(--color-text-tertiary);
	}
</style>
