<script lang="ts">
	import { Brain, Zap, AlertCircle } from 'lucide-svelte';
	import type { DatabaseStatus } from '$lib/types';

	let { status, analysisActive, analysisStatus, analysisProgress, onRunBatchSynthesis } = $props<{
		status: DatabaseStatus | null;
		analysisActive: boolean;
		analysisStatus: string;
		analysisProgress: number;
		onRunBatchSynthesis: () => void;
	}>();
</script>

<section class="center-card synthesis-engine">
	<header>
		<Brain size={18} />
		<div class="header-content">
			<h3>Cognitive Synthesis</h3>
			{#if analysisActive}
				<div class="analysis-progress">
					<span class="status-text">{analysisStatus}</span>
					<div class="progress-bar-bg">
						<div class="progress-bar-fill" style="width: {analysisProgress}%"></div>
					</div>
				</div>
			{:else}
				<div class="model-meta">
					<span>Gemma 4 (Local LLM)</span>
					<div class="actions">
						<button
							class="text-btn"
							onclick={onRunBatchSynthesis}
							disabled={!status || status.analyzed_records === status.completed_count}
						>
							<Zap size={14} /> Batch Synthesis
						</button>
					</div>
				</div>
			{/if}
		</div>
	</header>

	{#if status}
		<div class="diag-metrics">
			<div class="metric">
				<span>Audit Ready</span>
				<strong
					class={status.analyzed_records > (status.completed_count || 0) ? 'text-warning' : ''}
				>
					{status.analyzed_records}
				</strong>
			</div>
			<div class="metric">
				<span>Intelligence Synthesized</span>
				<strong>{status.completed_count || 0}</strong>
			</div>
			<div class="metric" style="grid-column: span 2;">
				<span>Inference Pipeline</span>
				<strong>Sequential Local LLM (VRAM Safe)</strong>
			</div>
		</div>
	{:else}
		<div class="loading-state">Syncing synthesis status...</div>
	{/if}

	<div class="hardware-warning-box">
		<div class="warning-header">
			<AlertCircle size={14} class="warning-icon" />
			<span>Resource Warning</span>
		</div>
		<p class="warning-text">
			Batch deep intelligence synthesis executes local LLM inference sequentially across all pending
			records. This action is extremely resource-intensive. Ensure your machine has active cooling,
			is connected to power, and avoid running other heavy workloads.
		</p>
	</div>
</section>

<style>
	.center-card {
		background: var(--color-bg-surface);
		border: 1px solid var(--color-border-subtle);
		border-radius: var(--radius-lg);
		padding: var(--space-5xl);
		display: flex;
		flex-direction: column;
		gap: var(--space-4xl);
	}

	.center-card header {
		display: flex;
		align-items: center;
		gap: var(--space-xl);
		color: var(--color-text-secondary);
	}

	.center-card h3 {
		margin: 0;
		font-size: var(--text-lg);
		text-transform: uppercase;
		letter-spacing: 0.1em;
		font-weight: 700;
		flex: 1;
	}

	.header-content {
		display: flex;
		align-items: center;
		gap: var(--space-xl);
		width: 100%;
	}

	.model-meta {
		display: flex;
		align-items: center;
		justify-content: space-between;
		flex: 1;
		font-size: var(--text-base);
		color: var(--color-text-secondary);
	}

	.actions {
		display: flex;
		gap: var(--space-md);
	}

	.text-btn {
		background: none;
		border: none;
		color: var(--color-accent-primary);
		font-size: var(--text-sm);
		font-weight: 700;
		text-transform: uppercase;
		display: flex;
		align-items: center;
		gap: var(--space-sm);
		cursor: pointer;
		padding: 4px 8px;
		border-radius: var(--radius-xs);
		transition: background 0.2s;
	}

	.text-btn:hover {
		background: rgba(231, 196, 107, 0.1);
	}

	.text-btn:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	.diag-metrics {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: var(--space-4xl);
	}

	.metric {
		display: flex;
		flex-direction: column;
		gap: var(--space-xs);
	}

	.metric span {
		font-size: var(--text-sm);
		color: var(--color-text-tertiary);
		text-transform: uppercase;
	}

	.metric strong {
		font-size: 15px;
		color: var(--color-text-primary);
	}

	.text-warning {
		color: var(--color-accent-primary) !important;
	}

	.loading-state {
		padding: var(--space-4xl);
		text-align: center;
		color: var(--color-text-tertiary);
		font-style: italic;
	}

	.analysis-progress {
		display: flex;
		flex-direction: column;
		gap: var(--space-xs);
		flex: 1;
		align-items: flex-end;
	}

	.status-text {
		font-size: var(--text-sm);
		color: var(--color-text-secondary);
	}

	.analysis-progress .progress-bar-bg {
		width: 100%;
		height: 4px;
		background: rgba(255, 255, 255, 0.05);
		border-radius: var(--radius-2xs);
		overflow: hidden;
	}

	.analysis-progress .progress-bar-fill {
		height: 100%;
		background: var(--color-accent-primary);
		transition: width 0.2s ease;
	}

	.hardware-warning-box {
		margin-top: auto;
		background: rgba(243, 196, 107, 0.05);
		border: 1px solid rgba(243, 196, 107, 0.15);
		border-radius: var(--radius-md);
		padding: 12px 16px;
		display: flex;
		flex-direction: column;
		gap: var(--space-sm);
	}

	.warning-header {
		display: flex;
		align-items: center;
		gap: var(--space-md);
		color: var(--color-accent-primary);
		font-size: var(--text-sm);
		font-weight: 700;
		text-transform: uppercase;
		letter-spacing: 0.05em;
	}

	:global(.warning-icon) {
		color: var(--color-accent-primary);
	}

	.warning-text {
		margin: 0;
		font-size: var(--text-sm);
		line-height: 1.5;
		color: var(--color-text-secondary);
	}
</style>
