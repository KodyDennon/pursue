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

	.raw-text-block {
		background: #000;
		padding: 24px;
		border-radius: 12px;
		font-family: var(--font-mono);
		font-size: 12px;
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
		gap: 16px;
		color: var(--text-tertiary);
	}

	.pending-state button {
		background: var(--accent-primary);
		color: #000;
		border: none;
		padding: 8px 16px;
		border-radius: 6px;
		font-weight: 700;
		cursor: pointer;
	}
</style>
