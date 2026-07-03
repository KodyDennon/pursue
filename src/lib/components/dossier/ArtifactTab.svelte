<script lang="ts">
	import { Download, ExternalLink, HardDrive, Maximize2 } from 'lucide-svelte';
	import type { RecordSummary } from '$lib/types';

	let {
		record,
		resolvePath,
		revealLocal,
		openSource,
		download,
		setViewerOpen,
		compact = false
	} = $props<{
		record: RecordSummary;
		resolvePath: (path: string | null) => string;
		revealLocal?: () => void;
		openSource?: () => void;
		download: () => void;
		setViewerOpen?: (open: boolean) => void;
		compact?: boolean;
	}>();
</script>

<div class="artifact-container" class:compact>
	{#if record.local_path}
		<div class="artifact-actions">
			{#if !compact}
				{#if record.document_url}
					<button class="action-btn" onclick={openSource}>
						<ExternalLink size={14} /> Source
					</button>
				{/if}
				<button class="action-btn" onclick={revealLocal}>
					<HardDrive size={14} /> Reveal
				</button>
			{/if}
			<button class="action-btn accent" onclick={() => setViewerOpen?.(true)}>
				<Maximize2 size={14} /> Full View
			</button>
		</div>

		<div class="artifact-preview">
			<iframe src={resolvePath(record.local_path)} title="Evidence Document"></iframe>
		</div>
	{:else}
		<div class="pending-state">
			<Download size={48} />
			<h3>Local Artifact Missing</h3>
			<p>Target intelligence must be acquired from remote systems.</p>
			<button class="primary-btn" onclick={download}>DOWNLOAD SOURCE</button>
		</div>
	{/if}
</div>

<style>
	.artifact-container {
		padding: var(--space-7xl);
		display: flex;
		flex-direction: column;
		gap: var(--space-4xl);
		height: 100%;
	}

	.artifact-container.compact {
		padding: var(--space-3xl);
		gap: var(--space-xl);
	}

	.artifact-actions {
		display: flex;
		gap: var(--space-xl);
		justify-content: flex-end;
	}

	.action-btn {
		background: rgba(255, 255, 255, 0.05);
		border: 1px solid var(--border-subtle);
		color: var(--text-secondary);
		padding: 6px 12px;
		border-radius: var(--radius-xs);
		font-size: var(--text-sm);
		font-weight: 700;
		display: flex;
		align-items: center;
		gap: var(--space-md);
		cursor: pointer;
		transition: all 0.2s;
	}

	.action-btn:hover {
		background: rgba(255, 255, 255, 0.1);
		color: var(--text-primary);
	}

	.action-btn.accent {
		border-color: var(--accent-primary-glow);
		color: var(--accent-primary);
	}

	.artifact-preview {
		flex: 1;
		background: #000;
		border-radius: var(--radius-md);
		overflow: hidden;
		border: 1px solid var(--border-subtle);
		min-height: 400px;
	}

	.artifact-preview iframe {
		width: 100%;
		height: 100%;
		border: none;
	}

	.pending-state {
		flex: 1;
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		text-align: center;
		gap: var(--space-3xl);
		color: var(--text-tertiary);
		border: 2px dashed var(--border-subtle);
		border-radius: var(--radius-md);
	}

	.pending-state h3 {
		color: var(--text-secondary);
	}

	.pending-state p {
		font-size: var(--text-base);
		max-width: 200px;
	}

	.primary-btn {
		background: var(--accent-primary);
		color: #000;
		border: none;
		padding: 10px 20px;
		border-radius: var(--radius-sm);
		font-weight: 800;
		font-size: var(--text-sm);
		cursor: pointer;
		margin-top: var(--space-md);
	}
</style>
