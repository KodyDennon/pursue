<script lang="ts">
	import { SvelteMap } from 'svelte/reactivity';
	import type { RecordSummary } from '$lib/types';

	let { records } = $props<{ records: RecordSummary[] }>();

	// Real logic: Group records by agency to visually map the relationships
	// in absence of a dedicated entity query command.
	const agencyMap = $derived.by(() => {
		const map = new SvelteMap<string, RecordSummary[]>();
		for (const rec of records) {
			const a = rec.agency || 'Unknown Origin';
			if (!map.has(a)) map.set(a, []);
			map.get(a)!.push(rec);
		}
		return Array.from(map.entries()).sort((a, b) => b[1].length - a[1].length);
	});
</script>

<div class="link-analysis">
	<div class="la-header">
		<h2>Agency Network Matrix</h2>
		<p>Visualizing intelligence nodes and document distribution.</p>
	</div>

	<div class="matrix-grid">
		{#each agencyMap as [agency, items] (agency)}
			<div class="node-cluster glass-panel">
				<div class="cluster-head">
					<h3>{agency}</h3>
					<span class="node-count">{items.length} Records</span>
				</div>
				<div class="link-lines">
					{#each items.slice(0, 10) as item (item.id)}
						<div class="link-item">
							<span class="dot"></span>
							<span class="title">{item.title}</span>
							<span
								class="status"
								class:completed={item.analysis_status === 'completed'}
								class:indexed={item.analysis_status === 'indexed'}
							></span>
						</div>
					{/each}
					{#if items.length > 10}
						<div class="more-link">... and {items.length - 10} more links</div>
					{/if}
				</div>
			</div>
		{/each}
	</div>
</div>

<style>
	.link-analysis {
		padding: var(--space-7xl);
		height: 100%;
		overflow-y: auto;
	}

	.la-header {
		margin-bottom: var(--space-7xl);
	}

	.la-header h2 {
		font-size: var(--text-4xl);
		color: var(--text-primary);
	}

	.la-header p {
		color: var(--text-secondary);
	}

	.matrix-grid {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(360px, 1fr));
		gap: var(--space-5xl);
	}

	.node-cluster {
		padding: var(--space-5xl);
		background: rgba(16, 17, 20, 0.8);
		border: 1px solid var(--border-subtle);
	}

	.cluster-head {
		display: flex;
		justify-content: space-between;
		align-items: center;
		margin-bottom: var(--space-4xl);
		padding-bottom: var(--space-xl);
		border-bottom: 1px solid rgba(255, 255, 255, 0.05);
	}

	.cluster-head h3 {
		font-size: var(--text-xl);
		color: var(--accent-primary);
	}

	.node-count {
		font-size: var(--text-sm);
		background: rgba(255, 255, 255, 0.1);
		padding: 4px 8px;
		border-radius: var(--radius-md);
	}

	.link-lines {
		display: flex;
		flex-direction: column;
		gap: var(--space-xl);
	}

	.link-item {
		display: flex;
		align-items: center;
		gap: var(--space-xl);
		font-size: var(--text-md);
	}

	.dot {
		width: 6px;
		height: 6px;
		border-radius: 50%;
		background: var(--text-secondary);
	}

	.title {
		flex: 1;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
		color: var(--text-primary);
	}

	.status {
		width: 8px;
		height: 8px;
		border-radius: 50%;
		background: rgba(255, 255, 255, 0.1);
	}

	.status.completed {
		background: var(--accent-success);
		box-shadow: 0 0 8px var(--accent-success);
	}

	.status.indexed {
		background: var(--color-accent-info);
		box-shadow: 0 0 8px rgba(50, 150, 255, 0.5);
	}

	.more-link {
		font-size: var(--text-sm);
		color: var(--text-secondary);
		text-align: center;
		margin-top: var(--space-md);
		font-style: italic;
	}
</style>
