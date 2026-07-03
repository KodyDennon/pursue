<script lang="ts">
	import type { AnalysisChunk } from '$lib/types';

	let { chunks } = $props<{
		chunks: AnalysisChunk[];
	}>();
</script>

<div class="view-padding">
	<header class="section-head"><span class="prefix">SEMANTIC CHUNK MANIFEST</span></header>
	<div class="chunk-list">
		{#each chunks as chunk (chunk.id)}
			<div class="chunk-card">
				<span class="c-idx">CHUNK_{chunk.chunk_index.toString().padStart(3, '0')}</span>
				<p>{chunk.text}</p>
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
		color: var(--text-tertiary);
	}

	.chunk-list {
		display: flex;
		flex-direction: column;
		gap: var(--space-3xl);
	}

	.chunk-card {
		background: rgba(255, 255, 255, 0.02);
		border: 1px solid var(--border-subtle);
		padding: var(--space-3xl);
		border-radius: var(--radius-base);
	}

	.c-idx {
		font-family: var(--font-mono);
		font-size: var(--text-xs);
		color: var(--accent-primary);
		margin-bottom: var(--space-md);
		display: block;
	}

	.chunk-card p {
		margin: 0;
		font-size: var(--text-md);
		line-height: 1.6;
		color: var(--text-secondary);
	}
</style>
