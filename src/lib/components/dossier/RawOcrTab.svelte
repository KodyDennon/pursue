<script lang="ts">
	import { FileText } from 'lucide-svelte';
	import type { AnalysisReport } from '$lib/types';

	let { analysis, runFoundationIndexing } = $props<{
		analysis: AnalysisReport | null;
		runFoundationIndexing: () => void;
	}>();
</script>

<div class="view-padding">
	{#if analysis?.ocr_text}
		<header class="section-head"><span class="prefix">FOUNDATION OCR LOG</span></header>
		<pre class="raw-text-block">{analysis.ocr_text}</pre>
	{:else}
		<div class="pending-state">
			<FileText size={48} />
			<h3>No Foundation Index</h3>
			<button onclick={runFoundationIndexing}>Audit Index</button>
		</div>
	{/if}
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

	.raw-text-block {
		background: #000;
		padding: var(--space-5xl);
		border-radius: var(--radius-md);
		font-family: var(--font-mono);
		font-size: var(--text-base);
		line-height: 1.8;
		white-space: pre-wrap;
		color: var(--text-secondary);
		border: 1px solid var(--border-subtle);
	}

	.pending-state {
		height: 400px;
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		text-align: center;
		gap: var(--space-3xl);
		color: var(--text-tertiary);
	}

	.pending-state button {
		background: var(--accent-primary);
		color: #000;
		border: none;
		padding: 8px 16px;
		border-radius: var(--radius-sm);
		font-weight: 700;
		cursor: pointer;
	}
</style>
