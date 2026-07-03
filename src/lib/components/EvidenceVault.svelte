<script lang="ts">
	import { onMount } from 'svelte';
	import { Database, ShieldCheck, HardDrive, FileText, AlertTriangle } from 'lucide-svelte';
	import { formatBytes } from '$lib/utils';
	import { vaultStore } from '$lib/stores/vaultStore.svelte';
	import { settingsStore } from '$lib/stores/settingsStore.svelte';
	import { intelligenceStore } from '$lib/stores/intelligenceStore.svelte';

	onMount(() => {
		vaultStore.init();
		settingsStore.init();
		intelligenceStore.loadStatus();
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
			{#if vaultStore.busy && vaultStore.verifyStatusText}
				<div class="verify-progress-container">
					<span class="verify-status">{vaultStore.verifyStatusText}</span>
					<div class="progress-bar-bg">
						<div class="progress-bar-fill" style="width: {vaultStore.verifyProgress}%"></div>
					</div>
				</div>
			{/if}
			<button
				class="integrity-btn"
				onclick={() => vaultStore.runIntegrityCheck()}
				disabled={vaultStore.busy}
			>
				<ShieldCheck size={16} /> Integrity Sweep
			</button>
		</div>
	</header>

	<div class="vault-grid">
		<section class="stat-card">
			<div class="stat-icon"><FileText size={18} /></div>
			<div class="stat-body">
				<span class="label">Total Intelligence Records</span>
				<span class="value">{intelligenceStore.status?.total_count || 0}</span>
			</div>
		</section>

		<section class="stat-card">
			<div class="stat-icon"><HardDrive size={18} /></div>
			<div class="stat-body">
				<span class="label">Local Storage Used</span>
				<span class="value">{formatBytes(intelligenceStore.status?.artifact_bytes || 0)}</span>
				<div class="storage-bar">
					<div
						class="fill"
						style="width: {((intelligenceStore.status?.local_records || 0) /
							(intelligenceStore.status?.total_records || 1)) *
							100}%"
					></div>
				</div>
				<span class="sub-label"
					>{intelligenceStore.status?.local_records || 0} Artifacts cached locally</span
				>
			</div>
		</section>

		<section class="stat-card warning">
			<div class="stat-icon"><AlertTriangle size={18} /></div>
			<div class="stat-body">
				<span class="label">Intelligence Pipeline</span>
				<div class="pipeline-stats">
					<div class="p-item">
						<span class="p-label">Pending</span>
						<span class="p-value">{intelligenceStore.status?.pending_count || 0}</span>
					</div>
					<div class="p-item">
						<span class="p-label">Indexed</span>
						<span class="p-value highlight-blue"
							>{(intelligenceStore.status?.analyzed_records || 0) -
								(intelligenceStore.status?.completed_count || 0)}</span
						>
					</div>
					<div class="p-item">
						<span class="p-label">Completed</span>
						<span class="p-value highlight-green"
							>{intelligenceStore.status?.completed_count || 0}</span
						>
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
					class:active={settingsStore.agentSettings.auto_sync}
					onclick={() => {
						settingsStore.agentSettings.auto_sync = !settingsStore.agentSettings.auto_sync;
						settingsStore.saveAgentSettings();
					}}
					aria-label="Auto-Retrieval Toggle"
				></button>
			</div>
			<div class="config-item">
				<div class="text">
					<strong>Encrypted Artifact Storage</strong>
					<span
						>{vaultStore.encryptionStatus?.enabled
							? `Vault files are stored with ${vaultStore.encryptionStatus.algorithm} at rest.`
							: 'Vault encryption status is unavailable.'}</span
					>
				</div>
				<div class="status-tag">{vaultStore.encryptionStatus?.enabled ? 'SECURE' : 'UNKNOWN'}</div>
			</div>
		</div>
	</div>
</div>

<style>
	.evidence-vault {
		display: flex;
		flex-direction: column;
		gap: var(--space-7xl);
		padding: var(--space-7xl);
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
		gap: var(--space-4xl);
		align-items: center;
	}

	.header-info h2 {
		font-size: var(--text-4xl);
		margin: 0;
	}

	.header-info p {
		color: var(--color-text-secondary);
		font-size: var(--text-lg);
		margin: 4px 0 0 0;
	}

	.actions-wrapper {
		display: flex;
		align-items: center;
		gap: var(--space-3xl);
	}

	.verify-progress-container {
		display: flex;
		flex-direction: column;
		gap: var(--space-xs);
		align-items: flex-end;
		width: 200px;
	}

	.verify-status {
		font-size: var(--text-sm);
		color: var(--color-text-secondary);
		white-space: nowrap;
	}

	.integrity-btn {
		display: flex;
		align-items: center;
		gap: var(--space-md);
		padding: 10px 20px;
		background: rgba(255, 255, 255, 0.05);
		border: 1px solid var(--color-border-subtle);
		border-radius: var(--radius-md);
		color: var(--color-text-primary);
		font-size: var(--text-md);
		font-weight: 600;
		transition: var(--transition-fast);
	}

	.integrity-btn:hover:not(:disabled) {
		border-color: var(--color-accent-primary);
		background: rgba(231, 196, 107, 0.05);
	}

	.vault-grid {
		display: grid;
		grid-template-columns: repeat(3, 1fr);
		gap: var(--space-4xl);
	}

	.stat-card {
		background: rgba(255, 255, 255, 0.02);
		border: 1px solid var(--color-border-subtle);
		border-radius: var(--radius-lg);
		padding: var(--space-5xl);
		display: flex;
		gap: var(--space-4xl);
	}

	.stat-icon {
		width: 40px;
		height: 40px;
		border-radius: var(--radius-md);
		background: rgba(255, 255, 255, 0.05);
		display: flex;
		align-items: center;
		justify-content: center;
		color: var(--color-text-secondary);
		flex-shrink: 0;
	}

	.stat-body {
		display: flex;
		flex-direction: column;
		gap: var(--space-xs);
		flex: 1;
	}

	.stat-body .label {
		font-size: var(--text-base);
		text-transform: uppercase;
		letter-spacing: 0.05em;
		color: var(--color-text-tertiary);
	}

	.stat-body .value {
		font-size: var(--text-5xl);
		font-weight: 700;
		color: var(--color-text-primary);
	}

	.storage-bar {
		height: 4px;
		background: rgba(255, 255, 255, 0.05);
		border-radius: var(--radius-2xs);
		margin: 12px 0 8px;
		overflow: hidden;
	}

	.storage-bar .fill {
		height: 100%;
		background: var(--color-accent-primary);
	}

	.sub-label {
		font-size: var(--text-sm);
		color: var(--color-text-secondary);
	}

	.stat-card.warning .stat-icon {
		color: #facc15;
		background: rgba(250, 204, 21, 0.1);
	}
	.stat-card.warning .desc {
		font-size: var(--text-sm);
		color: var(--color-text-tertiary);
		margin: 12px 0 0 0;
		line-height: 1.4;
	}

	.pipeline-stats {
		display: grid;
		grid-template-columns: repeat(3, 1fr);
		gap: var(--space-xl);
		margin-top: var(--space-md);
	}

	.p-item {
		display: flex;
		flex-direction: column;
		gap: var(--space-2xs);
	}

	.p-label {
		font-size: var(--text-xs);
		color: var(--color-text-tertiary);
		text-transform: uppercase;
	}

	.p-value {
		font-size: var(--text-2xl);
		font-weight: 700;
		color: var(--color-text-primary);
	}

	.highlight-blue {
		color: var(--color-accent-info);
	}
	.highlight-green {
		color: var(--color-accent-success);
	}

	.vault-management h3 {
		font-size: var(--text-lg);
		text-transform: uppercase;
		letter-spacing: 0.1em;
		color: var(--color-text-secondary);
		margin: 0 0 20px 0;
	}

	.config-list {
		display: flex;
		flex-direction: column;
		gap: var(--space-xl);
	}

	.config-item {
		display: flex;
		justify-content: space-between;
		align-items: center;
		padding: 16px 20px;
		background: rgba(0, 0, 0, 0.2);
		border: 1px solid var(--color-border-subtle);
		border-radius: var(--radius-md);
	}

	.config-item .text {
		display: flex;
		flex-direction: column;
		gap: var(--space-xs);
	}

	.config-item strong {
		font-size: var(--text-lg);
		color: var(--color-text-primary);
	}
	.config-item span {
		font-size: var(--text-base);
		color: var(--color-text-tertiary);
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
		background: var(--color-accent-primary);
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
		font-size: var(--text-xs);
		font-weight: 700;
		color: var(--color-accent-success);
		background: rgba(77, 243, 169, 0.1);
		padding: 2px 8px;
		border-radius: var(--radius-xs);
	}
</style>
