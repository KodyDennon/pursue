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
			<span class="d-val"
				>{(settingsStore.status?.artifact_bytes || 0) > 0 ? 'Active' : 'Standby'}</span
			>
		</div>
		<label class="perf-toggle">
			<input
				type="checkbox"
				bind:checked={settingsStore.performanceMode}
				onchange={() => settingsStore.savePerformanceMode()}
			/>
			<span>
				<strong>Performance Mode</strong>
				<small>Reduce expensive blur, glow, and animation effects for Windows workstations.</small>
			</span>
		</label>
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

	.section-desc {
		font-size: var(--text-md);
		color: var(--color-text-secondary);
		line-height: 1.6;
		margin: 0;
	}

	.data-item {
		display: flex;
		flex-direction: column;
		gap: var(--space-md);
	}

	.perf-toggle {
		display: flex;
		align-items: flex-start;
		gap: var(--space-xl);
		padding: var(--space-xl);
		border: 1px solid var(--color-border-subtle);
		border-radius: var(--radius-sm);
		background: rgba(255, 255, 255, 0.03);
		cursor: pointer;
	}

	.perf-toggle input {
		margin-top: 3px;
	}

	.perf-toggle span {
		display: flex;
		flex-direction: column;
		gap: var(--space-xs);
	}

	.perf-toggle strong {
		font-size: var(--text-md);
		color: var(--color-text-primary);
	}

	.perf-toggle small {
		font-size: var(--text-sm);
		line-height: 1.4;
		color: var(--color-text-tertiary);
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

	.s-footer {
		padding: 16px 24px;
		background: rgba(255, 255, 255, 0.02);
		border-top: 1px solid var(--color-border-subtle);
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
		border: 1px solid transparent;
		transition: all 0.2s;
	}

	.s-btn.danger-outline {
		background: transparent;
		color: var(--accent-error, var(--color-accent-danger));
		border: 1px solid rgba(243, 77, 77, 0.4);
	}

	.s-btn:hover {
		filter: brightness(1.1);
		transform: translateY(-1px);
	}

	:global(.accent-icon) {
		color: var(--color-accent-primary);
	}
</style>
