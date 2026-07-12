<script lang="ts">
	import { onMount } from 'svelte';
	import { invoke } from '@tauri-apps/api/core';
	import { HardDrive, Trash2, FolderOpen, FolderInput, Undo2 } from 'lucide-svelte';
	import { settingsStore } from '$lib/stores/settingsStore.svelte';
	import { formatBytes } from '$lib/utils';

	let logPath = $state('');

	const migrationPercent = $derived(
		settingsStore.migration?.bytes_total
			? Math.min(
					100,
					Math.round(
						((settingsStore.migration.bytes_copied ?? 0) / settingsStore.migration.bytes_total) *
							100
					)
				)
			: 0
	);

	onMount(async () => {
		try {
			logPath = await invoke<string>('get_log_path');
		} catch (e) {
			console.error('Failed to get log path', e);
		}
	});

	async function openLogsDir() {
		try {
			await invoke('open_logs_directory');
		} catch (e) {
			console.error('Failed to open logs directory', e);
		}
	}
</script>

<section class="settings-section glass-panel">
	<div class="s-header">
		<HardDrive size={18} class="accent-icon" />
		<h3>Data Environment</h3>
	</div>
	<div class="s-body">
		<div class="data-item">
			<span class="d-label">Storage Location</span>
			<code class="d-val">{settingsStore.storageLocation?.effective_root || 'Loading...'}</code>
			<span class="d-val">
				{#if settingsStore.storageLocation?.is_fallback}
					Configured location is unreachable — running from the default location this session
				{:else if settingsStore.storageLocation?.is_custom}
					Custom location — database, evidence library, and models are stored here
				{:else}
					Default location — pick another folder to route all storage to a different drive
				{/if}
			</span>
			{#if settingsStore.migration}
				<div class="usage-bar">
					<div class="usage-fill" style="width: {migrationPercent}%"></div>
				</div>
				<span class="d-val">
					{#if settingsStore.migration.status === 'copying'}
						Copying data... {formatBytes(settingsStore.migration.bytes_copied ?? 0)} of {formatBytes(
							settingsStore.migration.bytes_total ?? 0
						)}
					{:else if settingsStore.migration.status === 'error'}
						Migration failed — restarting on the previous location
					{:else}
						Finishing up — the application will restart
					{/if}
				</span>
			{:else}
				<div class="location-buttons">
					<button
						class="s-btn"
						onclick={() => settingsStore.changeStorageLocation()}
						disabled={settingsStore.busy === 'storage'}
					>
						<FolderInput size={14} />
						Change Location...
					</button>
					{#if settingsStore.storageLocation?.is_custom}
						<button
							class="s-btn"
							onclick={() =>
								settingsStore.changeStorageLocation(settingsStore.storageLocation?.default_root)}
							disabled={settingsStore.busy === 'storage'}
						>
							<Undo2 size={14} />
							Restore Default Location
						</button>
					{/if}
				</div>
			{/if}
		</div>
		<div class="data-item">
			<span class="d-label">Intelligence Database (SQLite)</span>
			<code class="d-val">{settingsStore.status?.database_path || 'Loading...'}</code>
			<span class="d-val">{formatBytes(settingsStore.status?.database_bytes || 0)} total usage</span
			>
		</div>
		<div class="data-item">
			<span class="d-label">System Log File</span>
			<code class="d-val">{logPath || 'Loading...'}</code>
			<span class="d-val">Records engine activities, warnings, and error diagnostics</span>
		</div>
		<div class="data-item">
			<span class="d-label">Evidence Library Size</span>
			<div class="usage-bar">
				<div
					class="usage-fill"
					style="width: {Math.min(
						100,
						(settingsStore.status?.artifact_bytes || 0) / 1024 / 1024 / 500
					) * 100}%"
				></div>
			</div>
			<span class="d-val"
				>{formatBytes(settingsStore.status?.artifact_bytes || 0)} across {settingsStore.status
					?.artifact_count || 0} plaintext assets</span
			>
		</div>
		<div class="data-item">
			<span class="d-label">Data Security Strategy</span>
			<span class="d-val">Database & Graph: Protected Boundary</span>
			<span class="d-val">Evidence Files: Native Plaintext</span>
		</div>
	</div>
	<footer class="s-footer">
		<div class="footer-buttons">
			<button class="s-btn" onclick={openLogsDir}>
				<FolderOpen size={14} />
				Open Logs Directory
			</button>
			<button
				class="s-btn danger"
				onclick={() => settingsStore.clearCache()}
				disabled={settingsStore.busy === 'clear'}
			>
				<Trash2 size={14} />
				Clear Evidence Cache
			</button>
		</div>
	</footer>
</section>

<style>
	.settings-section {
		background: var(--color-bg-surface);
		border: 1px solid var(--color-border-subtle);
		border-radius: var(--radius-lg);
		display: flex;
		flex-direction: column;
	}

	.s-header {
		padding: var(--space-5xl);
		display: flex;
		align-items: center;
		gap: var(--space-3xl);
		border-bottom: 1px solid var(--color-border-subtle);
	}

	.s-header h3 {
		font-size: var(--text-xl);
		font-weight: 600;
		margin: 0;
	}

	.s-body {
		padding: var(--space-5xl);
		flex: 1;
		display: flex;
		flex-direction: column;
		gap: var(--space-5xl);
	}

	.data-item {
		display: flex;
		flex-direction: column;
		gap: var(--space-md);
	}

	.d-label {
		font-size: var(--text-sm);
		text-transform: uppercase;
		letter-spacing: 0.1em;
		color: var(--color-text-tertiary);
	}

	.d-val {
		font-size: var(--text-md);
		color: var(--color-text-primary);
		word-break: break-all;
	}

	code.d-val {
		background: rgba(0, 0, 0, 0.3);
		padding: 4px 8px;
		border-radius: var(--radius-xs);
		font-family: var(--font-mono);
	}

	.usage-bar {
		height: 6px;
		background: rgba(255, 255, 255, 0.05);
		border-radius: 3px;
		overflow: hidden;
	}

	.usage-fill {
		height: 100%;
		background: var(--color-accent-primary);
		box-shadow: 0 0 8px var(--color-accent-primary);
	}

	.s-footer {
		padding: 16px 24px;
		background: rgba(255, 255, 255, 0.02);
		border-top: 1px solid var(--color-border-subtle);
	}

	.footer-buttons {
		display: flex;
		gap: var(--space-xl);
		align-items: center;
	}

	.location-buttons {
		display: flex;
		gap: var(--space-xl);
		align-items: center;
		margin-top: var(--space-md);
	}

	.s-btn {
		display: flex;
		align-items: center;
		gap: var(--space-md);
		padding: 8px 16px;
		border-radius: var(--radius-sm);
		font-size: var(--text-base);
		font-weight: 700;
		cursor: pointer;
		border: 1px solid rgba(255, 255, 255, 0.15);
		background: rgba(255, 255, 255, 0.05);
		color: var(--color-text-primary);
		transition: all 0.2s;
	}

	.s-btn.danger {
		background: rgba(243, 77, 77, 0.1);
		color: var(--accent-error, var(--color-accent-danger));
		border: 1px solid rgba(243, 77, 77, 0.2);
	}

	.s-btn:hover {
		filter: brightness(1.1);
		transform: translateY(-1px);
	}

	:global(.accent-icon) {
		color: var(--color-accent-primary);
	}
</style>
