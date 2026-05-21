<script lang="ts">
	import { HardDrive, Trash2 } from 'lucide-svelte';
	import { settingsStore } from '$lib/stores/settingsStore.svelte';
	import { formatBytes } from '$lib/utils';
</script>

<section class="settings-section glass-panel">
	<div class="s-header">
		<HardDrive size={18} class="accent-icon" />
		<h3>Data Environment</h3>
	</div>
	<div class="s-body">
		<div class="data-item">
			<span class="d-label">Intelligence Database (SQLite)</span>
			<code class="d-val">{settingsStore.status?.database_path || 'Loading...'}</code>
			<span class="d-val">{formatBytes(settingsStore.status?.database_bytes || 0)} total usage</span>
		</div>
		<div class="data-item">
			<span class="d-label">Evidence Library Size</span>
			<div class="usage-bar">
				<div
					class="usage-fill"
					style="width: {Math.min(100, (settingsStore.status?.artifact_bytes || 0) / 1024 / 1024 / 500) * 100}%"
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
		<button
			class="s-btn danger"
			onclick={() => settingsStore.clearCache()}
			disabled={settingsStore.busy === 'clear'}
		>
			<Trash2 size={14} />
			Clear Evidence Cache
		</button>
	</footer>
</section>

<style>
	.settings-section {
		background: var(--bg-surface);
		border: 1px solid var(--border-subtle);
		border-radius: var(--radius-lg);
		display: flex;
		flex-direction: column;
	}

	.s-header {
		padding: 24px;
		display: flex;
		align-items: center;
		gap: 16px;
		border-bottom: 1px solid var(--border-subtle);
	}

	.s-header h3 {
		font-size: 16px;
		font-weight: 600;
		margin: 0;
	}

	.s-body {
		padding: 24px;
		flex: 1;
		display: flex;
		flex-direction: column;
		gap: 24px;
	}

	.data-item {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}

	.d-label {
		font-size: 11px;
		text-transform: uppercase;
		letter-spacing: 0.1em;
		color: var(--text-tertiary);
	}

	.d-val {
		font-size: 13px;
		color: var(--text-primary);
		word-break: break-all;
	}

	code.d-val {
		background: rgba(0, 0, 0, 0.3);
		padding: 4px 8px;
		border-radius: 4px;
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
		background: var(--accent-primary);
		box-shadow: 0 0 8px var(--accent-primary);
	}

	.s-footer {
		padding: 16px 24px;
		background: rgba(255, 255, 255, 0.02);
		border-top: 1px solid var(--border-subtle);
	}

	.s-btn {
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 8px 16px;
		border-radius: var(--radius-sm);
		font-size: 12px;
		font-weight: 700;
		cursor: pointer;
		border: 1px solid transparent;
		transition: all 0.2s;
	}

	.s-btn.danger {
		background: rgba(243, 77, 77, 0.1);
		color: var(--accent-error, #ff4d4d);
		border: 1px solid rgba(243, 77, 77, 0.2);
	}

	.s-btn:hover {
		filter: brightness(1.1);
		transform: translateY(-1px);
	}

	:global(.accent-icon) {
		color: var(--accent-primary);
	}
</style>
