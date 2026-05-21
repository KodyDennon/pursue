<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import {
		Download,
		Brain,
		DownloadCloud
	} from 'lucide-svelte';
	import { downloadStore } from '$lib/stores/downloadStore.svelte';
	import { settingsStore } from '$lib/stores/settingsStore.svelte';
	import { intelligenceStore } from '$lib/stores/intelligenceStore.svelte';
	import AssetList from './agent/AssetList.svelte';

	let { onComplete, onAnalyze } = $props<{
		onComplete?: () => void;
		onAnalyze?: () => void;
	}>();

	onMount(() => {
		downloadStore.init(onComplete);
		settingsStore.init();
		intelligenceStore.loadStatus();
	});

	onDestroy(() => downloadStore.destroy());

	function getProgress(job: any) {
		if (job.total === 0) return 0;
		return ((job.completed + job.failed) / (job.total - job.skipped)) * 100;
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
				<button class="agent-btn primary" onclick={() => downloadStore.startBulkDownload(onComplete)}>
					<Download size={14} /> Start Global Download
				</button>
				<button class="agent-btn secondary" onclick={() => intelligenceStore.reindexAll(onAnalyze)}>
					<Brain size={14} /> Neural Extraction
				</button>
			{:else}
				<button class="agent-btn danger" onclick={() => downloadStore.cancelDownload()}> Abort Operation </button>
			{/if}
		</div>
	</div>

	<div class="agent-settings-bar">
		<div class="toggle-group">
			<label class="switch-label">
				<input type="checkbox" bind:checked={settingsStore.agentSettings.auto_sync} onchange={() => settingsStore.saveAgentSettings()} />
				<span class="slider"></span>
				<span class="label-text">Auto-Ingestion Pipeline</span>
			</label>
			<label class="switch-label">
				<input type="checkbox" bind:checked={settingsStore.agentSettings.auto_analyze} onchange={() => settingsStore.saveAgentSettings()} />
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
					{downloadStore.report.job.completed + downloadStore.report.job.failed} / {downloadStore.report.job.total - downloadStore.report.job.skipped} Assets
				</span>
			</div>

			<div class="progress-bar-bg">
				<div class="progress-bar-fill" style="width: {getProgress(downloadStore.report.job)}%"></div>
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
					Agent is currently idle. Monitoring <strong>{intelligenceStore.status?.total_count ?? intelligenceStore.status?.total_records ?? 0}</strong> intelligence
					records.
				</p>
				<div class="stats-row">
					<div class="s-card">
						<span class="s-label">UNPROVISIONED</span>
						<span class="s-val">{(intelligenceStore.status?.total_count ?? intelligenceStore.status?.total_records ?? 0) - (intelligenceStore.status?.local_records ?? 0)}</span>
					</div>
					<div class="s-card">
						<span class="s-label">UNANALYZED</span>
						<span class="s-val">{(intelligenceStore.status?.total_count ?? intelligenceStore.status?.total_records ?? 0) - (intelligenceStore.status?.analyzed_records ?? 0)}</span>
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
		gap: 20px;
		padding: 24px;
		border-radius: var(--radius-lg);
		background: var(--bg-surface);
		border: 1px solid var(--border-subtle);
		height: 100%;
	}

	.agent-header {
		display: flex;
		justify-content: space-between;
		align-items: center;
		margin-bottom: 8px;
	}

	.agent-actions-top {
		display: flex;
		gap: 12px;
	}

	.agent-settings-bar {
		padding: 12px 16px;
		background: rgba(255, 255, 255, 0.02);
		border: 1px solid var(--border-subtle);
		border-radius: var(--radius-md);
		margin-bottom: 20px;
	}

	.toggle-group {
		display: flex;
		gap: 32px;
	}

	.switch-label {
		display: flex;
		align-items: center;
		gap: 12px;
		cursor: pointer;
		font-size: 11px;
		font-weight: 700;
		color: var(--text-secondary);
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
		background: var(--accent-primary);
	}
	input:checked + .slider::after {
		transform: translateX(14px);
	}

	.agent-info {
		display: flex;
		gap: 16px;
		align-items: center;
	}

	.agent-info h3 {
		font-size: 16px;
		font-weight: 600;
		margin: 0;
		color: var(--text-primary);
	}

	.agent-info p {
		font-size: 13px;
		color: var(--text-secondary);
		margin: 4px 0 0 0;
	}

	.accent-icon {
		color: var(--accent-primary);
	}

	.agent-btn {
		padding: 8px 18px;
		border-radius: var(--radius-sm);
		font-size: 13px;
		font-weight: 600;
		cursor: pointer;
		transition: var(--transition-fast);
		display: flex;
		align-items: center;
		gap: 8px;
	}

	.agent-btn.primary {
		background: var(--accent-primary);
		color: #000;
		border: none;
	}

	.agent-btn.secondary {
		background: rgba(255, 255, 255, 0.05);
		color: var(--text-primary);
		border: 1px solid var(--border-subtle);
	}

	.agent-btn.danger {
		background: rgba(255, 70, 70, 0.1);
		color: #ff4646;
		border: 1px solid rgba(255, 70, 70, 0.3);
	}

	.agent-btn:hover {
		filter: brightness(1.1);
	}

	.agent-progress {
		display: flex;
		flex-direction: column;
		gap: 12px;
	}

	.progress-stats {
		display: flex;
		justify-content: space-between;
		align-items: center;
	}

	.status-badge {
		font-size: 11px;
		text-transform: uppercase;
		font-weight: 700;
		padding: 2px 8px;
		border-radius: 4px;
		letter-spacing: 0.05em;
	}

	.status-badge.running {
		background: rgba(50, 150, 255, 0.2);
		color: #3296ff;
	}
	.status-badge.completed {
		background: rgba(0, 200, 100, 0.2);
		color: #00c864;
	}
	.status-badge.failed {
		background: rgba(255, 70, 70, 0.2);
		color: #ff4646;
	}

	.count {
		font-size: 12px;
		color: var(--text-secondary);
	}

	.progress-bar-bg {
		height: 6px;
		background: rgba(255, 255, 255, 0.05);
		border-radius: 3px;
		overflow: hidden;
	}

	.progress-bar-fill {
		height: 100%;
		background: var(--accent-primary);
		transition: width 0.4s ease;
		box-shadow: 0 0 10px var(--accent-primary);
	}

	.mini-stats {
		display: flex;
		gap: 16px;
		font-size: 11px;
		color: var(--text-secondary);
	}

	.mini-stats strong {
		color: var(--text-primary);
	}
	.text-error {
		color: #ff4646 !important;
	}

	.asset-list {
		flex: 1;
		overflow-y: auto;
		display: flex;
		flex-direction: column;
		gap: 4px;
		padding-right: 8px;
	}

	.asset-item {
		display: flex;
		gap: 12px;
		padding: 10px;
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
		background: var(--text-tertiary);
	}

	.asset-details {
		display: flex;
		flex-direction: column;
		gap: 2px;
	}

	.asset-title {
		font-size: 13px;
		color: var(--text-primary);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
		max-width: 400px;
	}

	.asset-meta {
		font-size: 11px;
		color: var(--text-tertiary);
	}

	.text-success {
		color: #00c864;
	}
	.text-accent {
		color: var(--accent-primary);
	}

	.agent-idle {
		flex: 1;
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 60px;
		text-align: center;
		border: 1px dashed var(--border-subtle);
		border-radius: var(--radius-md);
		background: rgba(0, 0, 0, 0.1);
	}

	.idle-content {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 16px;
	}

	.status-indicator {
		font-size: 10px;
		font-weight: 900;
		color: var(--text-tertiary);
		padding: 4px 12px;
		background: rgba(255, 255, 255, 0.05);
		border-radius: 20px;
		letter-spacing: 0.2em;
	}

	.idle-content p {
		color: var(--text-secondary);
		margin: 0;
	}

	.stats-row {
		display: flex;
		gap: 24px;
		margin-top: 8px;
	}

	.s-card {
		display: flex;
		flex-direction: column;
		gap: 4px;
	}

	.s-label {
		font-size: 9px;
		color: var(--text-tertiary);
		font-weight: 800;
	}

	.s-val {
		font-size: 24px;
		font-weight: 300;
		color: var(--text-primary);
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
