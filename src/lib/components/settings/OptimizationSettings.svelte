<script lang="ts">
	import { Cpu, Trash2 } from 'lucide-svelte';
	import { settingsStore } from '$lib/stores/settingsStore.svelte';
</script>

<section class="settings-section glass-panel">
	<div class="s-header">
		<Cpu size={18} class="accent-icon" />
		<h3>Hardware Optimization</h3>
	</div>
	<div class="s-body">
		<p class="section-desc">
			The Intelligence Engine automatically optimizes for your hardware tier. Currently running in
			<strong>Accelerated</strong> mode.
		</p>
		<div class="data-item">
			<span class="d-label">Neural Model Cache</span>
			<span class="d-val">{(settingsStore.status?.artifact_bytes || 0) > 0 ? 'Active' : 'Standby'}</span>
		</div>
	</div>
	<footer class="s-footer">
		<button
			class="s-btn danger-outline"
			onclick={() => settingsStore.purgeSystem()}
			disabled={settingsStore.busy === 'purge'}
		>
			<Trash2 size={14} /> Absolute System Purge
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

	.section-desc {
		font-size: 13px;
		color: var(--text-secondary);
		line-height: 1.6;
		margin: 0;
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

	.s-btn.danger-outline {
		background: transparent;
		color: var(--accent-error, #ff4d4d);
		border: 1px solid rgba(243, 77, 77, 0.4);
	}

	.s-btn:hover {
		filter: brightness(1.1);
		transform: translateY(-1px);
	}

	:global(.accent-icon) {
		color: var(--accent-primary);
	}
</style>
