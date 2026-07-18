<script lang="ts">
	import { onMount } from 'svelte';
	import { Download, Brain, DownloadCloud } from 'lucide-svelte';
	import { downloadStore } from '$lib/stores/downloadStore.svelte';
	import { settingsStore } from '$lib/stores/settingsStore.svelte';
	import { intelligenceStore } from '$lib/stores/intelligenceStore.svelte';
	import type { BulkDownloadStatus } from '$lib/types';
	import AssetList from './agent/AssetList.svelte';

	let { onComplete, onAnalyze } = $props<{
		onComplete?: () => void;
		onAnalyze?: () => void;
	}>();

	onMount(() => {
		// downloadStore's lifecycle is owned by +page.svelte (app level): it must keep
		// downloading when the user switches views. This view-scoped init used to also
		// destroy() the worker on unmount, silently stopping every active download the
		// moment the user left the Agent tab. Re-init here is a cheap no-op when polling
		// is already active, and picks the job up if the user lands here first.
		downloadStore.init(onComplete);
		settingsStore.init();
		intelligenceStore.loadStatus();
	});

	function getProgress(job: BulkDownloadStatus) {
		const actionable = job.total - job.skipped;
		if (actionable <= 0) return 0;
		return ((job.completed + job.failed) / actionable) * 100;
	}
</script>

<div class="agent-container glass-panel">
	<div class="agent-header">
		<div class="agent-info">
			<DownloadCloud size={20} class="accent-icon" />
			<div class="text">
				<h3>Ingestion Agent</h3>
				<p>Automated retrieval of official source documentation and media assets.</p>
			</div>
		</div>

		<div class="agent-actions-top">
			{#if !downloadStore.activeJobId || (downloadStore.report && downloadStore.report.job.status !== 'running' && downloadStore.report.job.status !== 'queued')}
				<button
					class="agent-btn primary"
					onclick={() => downloadStore.startBulkDownload(onComplete)}
				>
					<Download size={14} /> Start Global Download
				</button>
				<button class="agent-btn secondary" onclick={() => intelligenceStore.reindexAll(onAnalyze)}>
					<Brain size={14} /> Neural Extraction
				</button>
			{:else}
				<button class="agent-btn danger" onclick={() => downloadStore.cancelDownload()}>
					Abort Operation
				</button>
			{/if}
		</div>
	</div>

	<div class="agent-settings-bar">
		<div class="toggle-group">
			<label class="switch-label">
				<input
					type="checkbox"
					bind:checked={settingsStore.agentSettings.auto_sync}
					onchange={() => settingsStore.saveAgentSettings()}
				/>
				<span class="slider"></span>
				<span class="label-text">Auto-Ingestion Pipeline</span>
			</label>
			<label class="switch-label">
				<input
					type="checkbox"
					bind:checked={settingsStore.agentSettings.auto_analyze}
					onchange={() => settingsStore.saveAgentSettings()}
				/>
				<span class="slider"></span>
				<span class="label-text">Neural Post-Processing</span>
			</label>
		</div>
	</div>

	{#if downloadStore.report}
		<div class="agent-progress">
			<div class="progress-stats">
				<span class="status-badge {downloadStore.report.job.status}">
					{downloadStore.report.job.status.replace('_', ' ')}
				</span>
				<span class="count">
					{downloadStore.report.job.completed + downloadStore.report.job.failed} / {downloadStore
						.report.job.total - downloadStore.report.job.skipped} Assets
				</span>
			</div>

			<div class="progress-bar-bg">
				<div
					class="progress-bar-fill"
					style="width: {getProgress(downloadStore.report.job)}%"
				></div>
			</div>

			<div class="mini-stats">
				<span>Completed: <strong>{downloadStore.report.job.completed}</strong></span>
				<span
					>Failed: <strong class={downloadStore.report.job.failed > 0 ? 'text-error' : ''}
						>{downloadStore.report.job.failed}</strong
					></span
				>
				<span>Skipped (Cached): <strong>{downloadStore.report.job.skipped}</strong></span>
			</div>
		</div>

		<AssetList report={downloadStore.report} />
	{:else}
		<div class="agent-idle">
			<div class="idle-content">
				<div class="status-indicator">STANDBY</div>
				<p>
					Agent is currently idle. Monitoring <strong
						>{intelligenceStore.status?.total_count ??
							intelligenceStore.status?.total_records ??
							0}</strong
					> intelligence records.
				</p>
				<div class="stats-row">
					<div class="s-card">
						<span class="s-label">UNPROVISIONED</span>
						<span class="s-val"
							>{(intelligenceStore.status?.total_count ??
								intelligenceStore.status?.total_records ??
								0) - (intelligenceStore.status?.local_records ?? 0)}</span
						>
					</div>
					<div class="s-card">
						<span class="s-label">UNANALYZED</span>
						<span class="s-val"
							>{(intelligenceStore.status?.total_count ??
								intelligenceStore.status?.total_records ??
								0) - (intelligenceStore.status?.analyzed_records ?? 0)}</span
						>
					</div>
				</div>
			</div>
		</div>
	{/if}
</div>

<style>
	.agent-container {
		display: flex;
		flex-direction: column;
		gap: var(--space-4xl);
		padding: var(--space-5xl);
		border-radius: var(--radius-lg);
		background: var(--color-bg-surface);
		border: 1px solid var(--color-border-subtle);
		height: 100%;
	}

	.agent-header {
		display: flex;
		justify-content: space-between;
		align-items: center;
		margin-bottom: var(--space-md);
	}

	.agent-actions-top {
		display: flex;
		gap: var(--space-xl);
	}

	.agent-settings-bar {
		padding: 12px 16px;
		background: rgba(255, 255, 255, 0.02);
		border: 1px solid var(--color-border-subtle);
		border-radius: var(--radius-md);
		margin-bottom: var(--space-4xl);
	}

	.toggle-group {
		display: flex;
		gap: var(--space-7xl);
	}

	.switch-label {
		display: flex;
		align-items: center;
		gap: var(--space-xl);
		cursor: pointer;
		font-size: var(--text-sm);
		font-weight: 700;
		color: var(--color-text-secondary);
		text-transform: uppercase;
		letter-spacing: 0.05em;
	}

	.switch-label input {
		display: none;
	}
	.slider {
		width: 28px;
		height: 14px;
		background: #2a2d35;
		border-radius: 10px;
		position: relative;
		transition: background 0.3s;
	}
	.slider::after {
		content: '';
		position: absolute;
		width: 10px;
		height: 10px;
		top: 2px;
		left: 2px;
		background: #fff;
		border-radius: 50%;
		transition: transform 0.3s;
	}
	input:checked + .slider {
		background: var(--color-accent-primary);
	}
	input:checked + .slider::after {
		transform: translateX(14px);
	}

	.agent-info {
		display: flex;
		gap: var(--space-3xl);
		align-items: center;
	}

	.agent-info h3 {
		font-size: var(--text-xl);
		font-weight: 600;
		margin: 0;
		color: var(--color-text-primary);
	}

	.agent-info p {
		font-size: var(--text-md);
		color: var(--color-text-secondary);
		margin: 4px 0 0 0;
	}

	.accent-icon {
		color: var(--color-accent-primary);
	}

	.agent-btn {
		padding: 8px 18px;
		border-radius: var(--radius-sm);
		font-size: var(--text-md);
		font-weight: 600;
		cursor: pointer;
		transition: var(--transition-fast);
		display: flex;
		align-items: center;
		gap: var(--space-md);
	}

	.agent-btn.primary {
		background: var(--color-accent-primary);
		color: #000;
		border: none;
	}

	.agent-btn.secondary {
		background: rgba(255, 255, 255, 0.05);
		color: var(--color-text-primary);
		border: 1px solid var(--color-border-subtle);
	}

	.agent-btn.danger {
		background: rgba(255, 70, 70, 0.1);
		color: var(--color-accent-danger);
		border: 1px solid rgba(255, 70, 70, 0.3);
	}

	.agent-btn:hover {
		filter: brightness(1.1);
	}

	.agent-progress {
		display: flex;
		flex-direction: column;
		gap: var(--space-xl);
	}

	.progress-stats {
		display: flex;
		justify-content: space-between;
		align-items: center;
	}

	.status-badge {
		font-size: var(--text-sm);
		text-transform: uppercase;
		font-weight: 700;
		padding: 2px 8px;
		border-radius: var(--radius-xs);
		letter-spacing: 0.05em;
	}

	.status-badge.running {
		background: rgba(50, 150, 255, 0.2);
		color: var(--color-accent-info);
	}
	.status-badge.completed {
		background: rgba(0, 200, 100, 0.2);
		color: var(--color-accent-success);
	}
	.status-badge.failed {
		background: rgba(255, 70, 70, 0.2);
		color: var(--color-accent-danger);
	}

	.count {
		font-size: var(--text-base);
		color: var(--color-text-secondary);
	}

	.progress-bar-bg {
		height: 6px;
		background: rgba(255, 255, 255, 0.05);
		border-radius: 3px;
		overflow: hidden;
	}

	.progress-bar-fill {
		height: 100%;
		background: var(--color-accent-primary);
		transition: width 0.4s ease;
		box-shadow: 0 0 10px var(--color-accent-primary);
	}

	.mini-stats {
		display: flex;
		gap: var(--space-3xl);
		font-size: var(--text-sm);
		color: var(--color-text-secondary);
	}

	.mini-stats strong {
		color: var(--color-text-primary);
	}
	.text-error {
		color: var(--color-accent-danger) !important;
	}

	.asset-list {
		flex: 1;
		overflow-y: auto;
		display: flex;
		flex-direction: column;
		gap: var(--space-xs);
		padding-right: var(--space-md);
	}

	.asset-item {
		display: flex;
		gap: var(--space-xl);
		padding: var(--space-lg);
		border-radius: var(--radius-sm);
		background: rgba(255, 255, 255, 0.02);
		border: 1px solid transparent;
		transition: var(--transition-fast);
	}

	.asset-item:hover {
		background: rgba(255, 255, 255, 0.04);
	}

	.asset-icon {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 16px;
	}

	.dot {
		width: 4px;
		height: 4px;
		border-radius: 50%;
		background: var(--color-text-tertiary);
	}

	.asset-details {
		display: flex;
		flex-direction: column;
		gap: var(--space-2xs);
	}

	.asset-title {
		font-size: var(--text-md);
		color: var(--color-text-primary);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
		max-width: 400px;
	}

	.asset-meta {
		font-size: var(--text-sm);
		color: var(--color-text-tertiary);
	}

	.text-success {
		color: var(--color-accent-success);
	}
	.text-accent {
		color: var(--color-accent-primary);
	}

	.agent-idle {
		flex: 1;
		display: flex;
		align-items: center;
		justify-content: center;
		padding: var(--space-9xl);
		text-align: center;
		border: 1px dashed var(--color-border-subtle);
		border-radius: var(--radius-md);
		background: rgba(0, 0, 0, 0.1);
	}

	.idle-content {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: var(--space-3xl);
	}

	.status-indicator {
		font-size: var(--text-xs);
		font-weight: 900;
		color: var(--color-text-tertiary);
		padding: 4px 12px;
		background: rgba(255, 255, 255, 0.05);
		border-radius: var(--radius-xl);
		letter-spacing: 0.2em;
	}

	.idle-content p {
		color: var(--color-text-secondary);
		margin: 0;
	}

	.stats-row {
		display: flex;
		gap: var(--space-5xl);
		margin-top: var(--space-md);
	}

	.s-card {
		display: flex;
		flex-direction: column;
		gap: var(--space-xs);
	}

	.s-label {
		font-size: var(--text-2xs);
		color: var(--color-text-tertiary);
		font-weight: 800;
	}

	.s-val {
		font-size: var(--text-4xl);
		font-weight: 300;
		color: var(--color-text-primary);
		font-family: var(--font-display);
	}

	:global(.spin) {
		animation: spin 1s linear infinite;
	}
	@keyframes spin {
		from {
			transform: rotate(0deg);
		}
		to {
			transform: rotate(360deg);
		}
	}
</style>
