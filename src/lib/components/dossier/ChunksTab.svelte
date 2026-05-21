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

	.chunk-list {
		display: flex;
		flex-direction: column;
		gap: 16px;
	}

	.chunk-card {
		background: rgba(255, 255, 255, 0.02);
		border: 1px solid var(--border-subtle);
		padding: 16px;
		border-radius: 8px;
	}

	.c-idx {
		font-family: var(--font-mono);
		font-size: 10px;
		color: var(--accent-primary);
		margin-bottom: 8px;
		display: block;
	}

	.chunk-card p {
		margin: 0;
		font-size: 13px;
		line-height: 1.6;
		color: var(--text-secondary);
	}
</style>
