<script lang="ts">
	import { HardDrive, Zap, RefreshCw } from 'lucide-svelte';
	import type { DatabaseStatus } from '$lib/types';
	import { formatBytes } from '$lib/utils';

	let {
		status,
		analysisActive,
		analysisStatus,
		analysisProgress,
		onReindexAll,
		onForceReprocessAll
	} = $props<{
		status: DatabaseStatus | null;
		analysisActive: boolean;
		analysisStatus: string;
		analysisProgress: number;
		onReindexAll: () => void;
		onForceReprocessAll: () => void;
	}>();
</script>

<section class="center-card vector">
	<header>
		<HardDrive size={18} />
		<div class="header-content">
			<h3>Vector Index Analytics</h3>
			{#if analysisActive}
				<div class="analysis-progress">
					<span class="status-text">{analysisStatus}</span>
					<div class="progress-bar-bg">
						<div class="progress-bar-fill" style="width: {analysisProgress}%"></div>
					</div>
				</div>
			{:else}
				<div class="model-meta">
					<span>BGE v1.5 (384d)</span>
					<div class="actions">
						<button class="text-btn" onclick={onReindexAll}>
							<Zap size={14} /> Audit Pending
						</button>
						<button class="text-btn danger" onclick={onForceReprocessAll}>
							<RefreshCw size={14} /> Force Re-Audit
						</button>
					</div>
				</div>
			{/if}
		</div>
	</header>
	{#if status}
		<div class="diag-metrics">
			<div class="metric">
				<span>Indexed Chunks</span>
				<strong>{status.vector_chunks}</strong>
			</div>
			<div class="metric">
				<span>Entity Associations</span>
				<strong>{status.entity_count}</strong>
			</div>
			<div class="metric">
				<span>Storage Overhead</span>
				<strong>{formatBytes(status.artifact_bytes)}</strong>
			</div>
			<div class="metric">
				<span>Search Engine</span>
				<strong>ONNX / Vector (BGE)</strong>
			</div>
			<div class="metric">
				<span>OCR Infrastructure</span>
				<strong>Native (Vision/Media)</strong>
			</div>
		</div>
	{:else}
		<div class="loading-state">Syncing index status...</div>
	{/if}
</section>

<style>
	.center-card {
		background: var(--bg-surface);
		border: 1px solid var(--border-subtle);
		border-radius: var(--radius-lg);
		padding: 24px;
		display: flex;
		flex-direction: column;
		gap: 20px;
	}

	.center-card header {
		display: flex;
		align-items: center;
		gap: 12px;
		color: var(--text-secondary);
	}

	.center-card h3 {
		margin: 0;
		font-size: 14px;
		text-transform: uppercase;
		letter-spacing: 0.1em;
		font-weight: 700;
		flex: 1;
	}

	.header-content {
		display: flex;
		align-items: center;
		gap: 12px;
		width: 100%;
	}

	.model-meta {
		display: flex;
		align-items: center;
		justify-content: space-between;
		flex: 1;
		font-size: 12px;
		color: var(--text-secondary);
	}

	.actions {
		display: flex;
		gap: 8px;
	}

	.text-btn {
		background: none;
		border: none;
		color: var(--accent-primary);
		font-size: 11px;
		font-weight: 700;
		text-transform: uppercase;
		display: flex;
		align-items: center;
		gap: 6px;
		cursor: pointer;
		padding: 4px 8px;
		border-radius: 4px;
		transition: background 0.2s;
	}

	.text-btn:hover {
		background: rgba(231, 196, 107, 0.1);
	}

	.text-btn.danger {
		color: var(--accent-error, #ff4d4d);
	}

	.text-btn.danger:hover {
		background: rgba(255, 77, 77, 0.1);
	}

	.diag-metrics {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: 20px;
	}

	.metric {
		display: flex;
		flex-direction: column;
		gap: 4px;
	}

	.metric span {
		font-size: 11px;
		color: var(--text-tertiary);
		text-transform: uppercase;
	}

	.metric strong {
		font-size: 15px;
		color: var(--text-primary);
	}

	.loading-state {
		padding: 20px;
		text-align: center;
		color: var(--text-tertiary);
		font-style: italic;
	}

	.analysis-progress {
		display: flex;
		flex-direction: column;
		gap: 4px;
		flex: 1;
		align-items: flex-end;
	}

	.status-text {
		font-size: 11px;
		color: var(--text-secondary);
	}

	.analysis-progress .progress-bar-bg {
		width: 100%;
		height: 4px;
		background: rgba(255, 255, 255, 0.05);
		border-radius: 2px;
		overflow: hidden;
	}

	.analysis-progress .progress-bar-fill {
		height: 100%;
		background: var(--accent-primary);
		transition: width 0.2s ease;
	}
</style>
