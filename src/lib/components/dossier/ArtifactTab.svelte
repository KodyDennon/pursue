<script lang="ts">
	import { Download } from 'lucide-svelte';
	import type { RecordSummary } from '$lib/types';

	let { record, resolvePath, download } = $props<{
		record: RecordSummary;
		resolvePath: (path: string | null) => string;
		download: () => void;
	}>();
</script>

<div class="view-padding">
	{#if record.local_path}
		<div class="artifact-preview">
			<iframe src={resolvePath(record.local_path)} title="Evidence Document"></iframe>
		</div>
	{:else}
		<div class="pending-state">
			<Download size={48} />
			<h3>Local Artifact Missing</h3>
			<button onclick={download}>Download Source</button>
		</div>
	{/if}
</div>

<style>
	.view-padding {
		padding: 32px;
	}

	.artifact-preview {
		height: 600px;
		background: #000;
		border-radius: 12px;
		overflow: hidden;
		border: 1px solid var(--border-subtle);
	}

	.artifact-preview iframe {
		width: 100%;
		height: 100%;
		border: none;
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
