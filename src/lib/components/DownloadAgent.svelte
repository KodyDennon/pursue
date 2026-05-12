<script lang="ts">
	import { invoke } from '@tauri-apps/api/core';
	import { onMount, onDestroy } from 'svelte';
	import {
		Loader2,
		Download,
		CheckCircle2,
		AlertCircle,
		Brain,
		DownloadCloud
	} from 'lucide-svelte';
	import type { BulkDownloadReport, BulkDownloadStatus, DatabaseStatus } from '$lib/types';
	import { addToast } from '$lib/toastStore';

	let { onComplete } = $props<{
		onComplete?: () => void;
		onAnalyze?: () => void;
	}>();

	let activeJobId = $state<string | null>(null);
	let report = $state<BulkDownloadReport | null>(null);
	let polling = $state(false);
	let pollInterval: ReturnType<typeof setInterval> | null = null;

	let agentSettings = $state({ auto_sync: true, auto_analyze: true });
	let stats = $state<DatabaseStatus | null>(null);

	async function loadSettings() {
		try {
			const s = await invoke<typeof agentSettings>('get_app_settings', { key: 'ingestion_agent' });
			if (s) agentSettings = s;
		} catch (e) {
			console.error(e);
		}
	}

	async function saveSettings() {
		try {
			await invoke('set_app_settings', { key: 'ingestion_agent', value: agentSettings });
		} catch (e) {
			console.error(e);
		}
	}

	async function loadStats() {
		try {
			stats = await invoke<DatabaseStatus>('get_evidence_stats');
		} catch (e) {
			console.error(e);
		}
	}

	async function startBulkDownload() {
		try {
			activeJobId = await invoke<string>('download_missing_records');
			startPolling();
			addToast({
				type: 'info',
				message: 'Ingestion Agent initiated bulk collection.',
				duration: 3000
			});
		} catch (e) {
			addToast({ type: 'error', message: `Agent failed: ${e}` });
		}
	}

	async function startBatchAnalysis() {
		try {
			await invoke('analyze_all_records');
			addToast({
				type: 'info',
				message: 'Neural Extraction Agent initiated batch re-indexing.',
				duration: 3000
			});
		} catch (e) {
			addToast({ type: 'error', message: `Analysis failed: ${e}` });
		}
	}

	async function cancelDownload() {
		if (!activeJobId) return;
		try {
			await invoke('cancel_bulk_download', { id: activeJobId });
		} catch (e) {
			console.error(e);
		}
	}

	async function fetchStatus() {
		if (!activeJobId) return;
		try {
			report = await invoke<BulkDownloadReport>('get_bulk_download_status', { id: activeJobId });
			if (
				report.job.status === 'completed' ||
				report.job.status === 'failed' ||
				report.job.status === 'cancelled'
			) {
				stopPolling();
				loadStats();
				if (onComplete) onComplete();
				if (report.job.status === 'completed' && agentSettings.auto_analyze) {
					addToast({
						type: 'info',
						message: 'Downloads complete. Auto-starting neural extraction...',
						duration: 5000
					});
					await invoke('analyze_all_records');
				}
			}
		} catch (e) {
			console.error('Poll failed', e);
			stopPolling();
		}
	}

	function startPolling() {
		if (polling) return;
		polling = true;
		fetchStatus();
		pollInterval = setInterval(fetchStatus, 2000);
	}

	function stopPolling() {
		polling = false;
		if (pollInterval) clearInterval(pollInterval);
	}

	onMount(async () => {
		loadSettings();
		loadStats();
		try {
			const latest = await invoke<BulkDownloadReport | null>('get_latest_download_job');
			if (latest) {
				activeJobId = latest.job.id;
				report = latest;
				startPolling();
			}
		} catch (e) {
			console.error('Failed to check for active job', e);
		}
	});

	onDestroy(() => stopPolling());

	function getProgress(job: BulkDownloadStatus) {
		if (job.total === 0) return 0;
		return ((job.completed + job.failed) / (job.total - job.skipped)) * 100;
	}

	function formatBytes(bytes: number) {
		if (bytes === 0) return '0 B';
		const k = 1024;
		const sizes = ['B', 'KB', 'MB', 'GB'];
		const i = Math.floor(Math.log(bytes) / Math.log(k));
		return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
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
			{#if !activeJobId || (report && report.job.status !== 'running' && report.job.status !== 'queued')}
				<button class="agent-btn primary" onclick={startBulkDownload}>
					<Download size={14} /> Start Global Download
				</button>
				<button class="agent-btn secondary" onclick={startBatchAnalysis}>
					<Brain size={14} /> Neural Extraction
				</button>
			{:else}
				<button class="agent-btn danger" onclick={cancelDownload}> Abort Operation </button>
			{/if}
		</div>
	</div>

	<div class="agent-settings-bar">
		<div class="toggle-group">
			<label class="switch-label">
				<input type="checkbox" bind:checked={agentSettings.auto_sync} onchange={saveSettings} />
				<span class="slider"></span>
				<span class="label-text">Auto-Ingestion Pipeline</span>
			</label>
			<label class="switch-label">
				<input type="checkbox" bind:checked={agentSettings.auto_analyze} onchange={saveSettings} />
				<span class="slider"></span>
				<span class="label-text">Neural Post-Processing</span>
			</label>
		</div>
	</div>

	{#if report}
		<div class="agent-progress">
			<div class="progress-stats">
				<span class="status-badge {report.job.status}">
					{report.job.status.replace('_', ' ')}
				</span>
				<span class="count">
					{report.job.completed + report.job.failed} / {report.job.total - report.job.skipped} Assets
				</span>
			</div>

			<div class="progress-bar-bg">
				<div class="progress-bar-fill" style="width: {getProgress(report.job)}%"></div>
			</div>

			<div class="mini-stats">
				<span>Completed: <strong>{report.job.completed}</strong></span>
				<span
					>Failed: <strong class={report.job.failed > 0 ? 'text-error' : ''}
						>{report.job.failed}</strong
					></span
				>
				<span>Skipped (Cached): <strong>{report.job.skipped}</strong></span>
			</div>
		</div>

		<div class="asset-list custom-scrollbar">
			{#each report.items as item (item.id)}
				<div class="asset-item {item.status}">
					<div class="asset-icon">
						{#if item.status === 'completed'}
							<CheckCircle2 size={14} class="text-success" />
						{:else if item.status === 'failed'}
							<AlertCircle size={14} class="text-error" />
						{:else if item.status === 'downloading'}
							<Loader2 size={14} class="spin text-accent" />
						{:else}
							<div class="dot"></div>
						{/if}
					</div>
					<div class="asset-details">
						<span class="asset-title">{item.title}</span>
						<span class="asset-meta">
							{#if item.status === 'completed'}
								{formatBytes(item.bytes_downloaded)} • Verified
							{:else if item.status === 'failed'}
								Error: {item.error || 'Unknown failure'}
							{:else}
								{item.status}...
							{/if}
						</span>
					</div>
				</div>
			{/each}
		</div>
	{:else}
		<div class="agent-idle">
			<div class="idle-content">
				<div class="status-indicator">STANDBY</div>
				<p>
					Agent is currently idle. Monitoring <strong>{stats?.total_count ?? stats?.total_records ?? 0}</strong> intelligence
					records.
				</p>
				<div class="stats-row">
					<div class="s-card">
						<span class="s-label">UNPROVISIONED</span>
						<span class="s-val">{(stats?.total_count ?? stats?.total_records ?? 0) - (stats?.local_records ?? 0)}</span>
					</div>
					<div class="s-card">
						<span class="s-label">UNANALYZED</span>
						<span class="s-val">{(stats?.total_count ?? stats?.total_records ?? 0) - (stats?.analyzed_records ?? 0)}</span>
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
