<script lang="ts">
	import { onMount } from 'svelte';
	import { invoke } from '@tauri-apps/api/core';
	import { listen } from '@tauri-apps/api/event';
	import { Database, ShieldCheck, HardDrive, FileText, AlertTriangle } from 'lucide-svelte';
	import { addToast } from '$lib/toastStore';
	import type { DatabaseStatus } from '$lib/types';
	import { formatBytes } from '$lib/utils';

	let stats = $state<DatabaseStatus & {
		pending_count?: number;
		indexed_count?: number;
		completed_count?: number;
	} | null>(null);

	let busy = $state(false);
	let verifyProgress = $state(0);
	let verifyStatusText = $state('');
	let agentSettings = $state({ auto_sync: true, auto_analyze: true });
	let encryptionStatus = $state<{ enabled: boolean; algorithm: string } | null>(null);

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
			stats = await invoke<DatabaseStatus & {
				pending_count?: number;
				indexed_count?: number;
				completed_count?: number;
			}>('get_evidence_stats');
			encryptionStatus = await invoke('get_vault_encryption_status');
		} catch (e) {
			console.error(e);
		}
	}

	async function runIntegrityCheck() {
		if (busy) return;
		busy = true;
		verifyProgress = 0;
		verifyStatusText = 'Initiating SHA-256 integrity sweep across vault...';
		addToast({ type: 'info', message: verifyStatusText, duration: 3000 });

		let unlisten: (() => void) | null = null;
		try {
			unlisten = await listen<{ current: number; total: number; status: string }>(
				'integrity-progress',
				(event) => {
					const { current, total, status } = event.payload;
					if (total > 0) {
						verifyProgress = (current / total) * 100;
					}
					if (status === 'completed') {
						verifyStatusText = 'Finalizing report...';
					} else {
						verifyStatusText = `Verifying artifact ${current} of ${total}...`;
					}
				}
			);

			const report = await invoke<{ verified: number; corrupted: number; missing: number }>(
				'verify_vault_integrity'
			);

			const corrupted = report.corrupted;
			const missing = report.missing;

			if (corrupted === 0 && missing === 0) {
				addToast({
					type: 'success',
					message: `Integrity check complete. All ${report.verified} local artifacts verified.`,
					duration: 5000
				});
			} else {
				addToast({
					type: 'error',
					message: `Integrity failure: ${corrupted} corrupted, ${missing} missing.`,
					duration: 8000
				});
			}
		} catch (e) {
			addToast({ type: 'error', message: `Integrity check failed: ${e}`, duration: 5000 });
		} finally {
			if (unlisten) unlisten();
			busy = false;
			verifyStatusText = '';
			verifyProgress = 0;
		}
	}

	onMount(() => {
		loadSettings();
		loadStats();
	});
</script>

<div class="evidence-vault glass-panel">
	<header class="vault-header">
		<div class="header-info">
			<Database size={24} class="accent-icon" />
			<div>
				<h2>Evidence Vault</h2>
				<p>Forensic storage and artifact lifecycle management.</p>
			</div>
		</div>
		<div class="actions-wrapper">
			{#if busy && verifyStatusText}
				<div class="verify-progress-container">
					<span class="verify-status">{verifyStatusText}</span>
					<div class="progress-bar-bg">
						<div class="progress-bar-fill" style="width: {verifyProgress}%"></div>
					</div>
				</div>
			{/if}
			<button class="integrity-btn" onclick={runIntegrityCheck} disabled={busy}>
				<ShieldCheck size={16} /> Integrity Sweep
			</button>
		</div>
	</header>

	<div class="vault-grid">
		<section class="stat-card">
			<div class="stat-icon"><FileText size={18} /></div>
			<div class="stat-body">
				<span class="label">Total Intelligence Records</span>
				<span class="value">{stats?.total_count || 0}</span>
			</div>
		</section>

		<section class="stat-card">
			<div class="stat-icon"><HardDrive size={18} /></div>
			<div class="stat-body">
				<span class="label">Local Storage Used</span>
				<span class="value">{formatBytes(stats?.total_size || 0)}</span>
				<div class="storage-bar">
					<div
						class="fill"
						style="width: {((stats?.local_records || 0) / (stats?.total_records || 1)) * 100}%"
						></div>
						</div>
						<span class="sub-label">{stats?.local_records || 0} Artifacts cached locally</span>			</div>
		</section>

		<section class="stat-card warning">
			<div class="stat-icon"><AlertTriangle size={18} /></div>
			<div class="stat-body">
				<span class="label">Intelligence Pipeline</span>
				<div class="pipeline-stats">
					<div class="p-item">
						<span class="p-label">Pending</span>
						<span class="p-value">{stats?.pending_count || 0}</span>
					</div>
					<div class="p-item">
						<span class="p-label">Indexed</span>
						<span class="p-value highlight-blue">{stats?.indexed_count || 0}</span>
					</div>
					<div class="p-item">
						<span class="p-label">Completed</span>
						<span class="p-value highlight-green">{stats?.completed_count || 0}</span>
					</div>
				</div>
				<p class="desc">Awaiting Gemma 4 neural extraction to reach 'Intelligence Ready' status.</p>
			</div>
		</section>
	</div>

	<div class="vault-management">
		<h3>Vault Configuration</h3>
		<div class="config-list">
			<div class="config-item">
				<div class="text">
					<strong>Auto-Retrieval Pipeline</strong>
					<span>Automatically download official sources when synced.</span>
				</div>
				<button
					class="toggle"
					class:active={agentSettings.auto_sync}
					onclick={() => {
						agentSettings.auto_sync = !agentSettings.auto_sync;
						saveSettings();
					}}
					aria-label="Auto-Retrieval Toggle"
				></button>
			</div>
			<div class="config-item">
				<div class="text">
					<strong>Encrypted Artifact Storage</strong>
					<span
						>{encryptionStatus?.enabled
							? `Vault files are stored with ${encryptionStatus.algorithm} at rest.`
							: 'Vault encryption status is unavailable.'}</span
					>
				</div>
				<div class="status-tag">{encryptionStatus?.enabled ? 'SECURE' : 'UNKNOWN'}</div>
			</div>
		</div>
	</div>
</div>

<style>
	.evidence-vault {
		display: flex;
		flex-direction: column;
		gap: 32px;
		padding: 32px;
		height: 100%;
		overflow-y: auto;
	}

	.vault-header {
		display: flex;
		justify-content: space-between;
		align-items: center;
	}

	.header-info {
		display: flex;
		gap: 20px;
		align-items: center;
	}

	.header-info h2 {
		font-size: 24px;
		margin: 0;
	}

	.header-info p {
		color: var(--text-secondary);
		font-size: 14px;
		margin: 4px 0 0 0;
	}

	.actions-wrapper {
		display: flex;
		align-items: center;
		gap: 16px;
	}

	.verify-progress-container {
		display: flex;
		flex-direction: column;
		gap: 4px;
		align-items: flex-end;
		width: 200px;
	}

	.verify-status {
		font-size: 11px;
		color: var(--text-secondary);
		white-space: nowrap;
	}

	.integrity-btn {
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 10px 20px;
		background: rgba(255, 255, 255, 0.05);
		border: 1px solid var(--border-subtle);
		border-radius: var(--radius-md);
		color: var(--text-primary);
		font-size: 13px;
		font-weight: 600;
		transition: var(--transition-fast);
	}

	.integrity-btn:hover:not(:disabled) {
		border-color: var(--accent-primary);
		background: rgba(231, 196, 107, 0.05);
	}

	.vault-grid {
		display: grid;
		grid-template-columns: repeat(3, 1fr);
		gap: 20px;
	}

	.stat-card {
		background: rgba(255, 255, 255, 0.02);
		border: 1px solid var(--border-subtle);
		border-radius: var(--radius-lg);
		padding: 24px;
		display: flex;
		gap: 20px;
	}

	.stat-icon {
		width: 40px;
		height: 40px;
		border-radius: 12px;
		background: rgba(255, 255, 255, 0.05);
		display: flex;
		align-items: center;
		justify-content: center;
		color: var(--text-secondary);
		flex-shrink: 0;
	}

	.stat-body {
		display: flex;
		flex-direction: column;
		gap: 4px;
		flex: 1;
	}

	.stat-body .label {
		font-size: 12px;
		text-transform: uppercase;
		letter-spacing: 0.05em;
		color: var(--text-tertiary);
	}

	.stat-body .value {
		font-size: 28px;
		font-weight: 700;
		color: var(--text-primary);
	}

	.storage-bar {
		height: 4px;
		background: rgba(255, 255, 255, 0.05);
		border-radius: 2px;
		margin: 12px 0 8px;
		overflow: hidden;
	}

	.storage-bar .fill {
		height: 100%;
		background: var(--accent-primary);
	}

	.sub-label {
		font-size: 11px;
		color: var(--text-secondary);
	}

	.stat-card.warning .stat-icon {
		color: #facc15;
		background: rgba(250, 204, 21, 0.1);
	}
	.stat-card.warning .desc {
		font-size: 11px;
		color: var(--text-tertiary);
		margin: 12px 0 0 0;
		line-height: 1.4;
	}

	.pipeline-stats {
		display: grid;
		grid-template-columns: repeat(3, 1fr);
		gap: 12px;
		margin-top: 8px;
	}

	.p-item {
		display: flex;
		flex-direction: column;
		gap: 2px;
	}

	.p-label {
		font-size: 10px;
		color: var(--text-tertiary);
		text-transform: uppercase;
	}

	.p-value {
		font-size: 18px;
		font-weight: 700;
		color: var(--text-primary);
	}

	.highlight-blue {
		color: #3296ff;
	}
	.highlight-green {
		color: var(--accent-success);
	}

	.vault-management h3 {
		font-size: 14px;
		text-transform: uppercase;
		letter-spacing: 0.1em;
		color: var(--text-secondary);
		margin: 0 0 20px 0;
	}

	.config-list {
		display: flex;
		flex-direction: column;
		gap: 12px;
	}

	.config-item {
		display: flex;
		justify-content: space-between;
		align-items: center;
		padding: 16px 20px;
		background: rgba(0, 0, 0, 0.2);
		border: 1px solid var(--border-subtle);
		border-radius: var(--radius-md);
	}

	.config-item .text {
		display: flex;
		flex-direction: column;
		gap: 4px;
	}

	.config-item strong {
		font-size: 14px;
		color: var(--text-primary);
	}
	.config-item span {
		font-size: 12px;
		color: var(--text-tertiary);
	}

	.toggle {
		width: 32px;
		height: 18px;
		background: #333;
		border-radius: 9px;
		position: relative;
		cursor: pointer;
	}
	.toggle.active {
		background: var(--accent-primary);
	}
	.toggle::after {
		content: '';
		position: absolute;
		top: 2px;
		left: 2px;
		width: 14px;
		height: 14px;
		background: white;
		border-radius: 50%;
		transition: transform 0.2s;
	}
	.toggle.active::after {
		transform: translateX(14px);
	}

	.status-tag {
		font-size: 10px;
		font-weight: 700;
		color: var(--accent-success);
		background: rgba(77, 243, 169, 0.1);
		padding: 2px 8px;
		border-radius: 4px;
	}
</style>
