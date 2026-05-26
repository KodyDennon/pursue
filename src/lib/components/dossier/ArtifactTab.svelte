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
		padding: 32px;
		display: flex;
		flex-direction: column;
		gap: 20px;
		height: 100%;
	}

	.artifact-container.compact {
		padding: 16px;
		gap: 12px;
	}

	.artifact-actions {
		display: flex;
		gap: 12px;
		justify-content: flex-end;
	}

	.action-btn {
		background: rgba(255, 255, 255, 0.05);
		border: 1px solid var(--border-subtle);
		color: var(--text-secondary);
		padding: 6px 12px;
		border-radius: 4px;
		font-size: 11px;
		font-weight: 700;
		display: flex;
		align-items: center;
		gap: 8px;
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
		border-radius: 12px;
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
		gap: 16px;
		color: var(--text-tertiary);
		border: 2px dashed var(--border-subtle);
		border-radius: 12px;
	}

	.pending-state h3 {
		color: var(--text-secondary);
	}

	.pending-state p {
		font-size: 12px;
		max-width: 200px;
	}

	.primary-btn {
		background: var(--accent-primary);
		color: #000;
		border: none;
		padding: 10px 20px;
		border-radius: 6px;
		font-weight: 800;
		font-size: 11px;
		cursor: pointer;
		margin-top: 8px;
	}
</style>
