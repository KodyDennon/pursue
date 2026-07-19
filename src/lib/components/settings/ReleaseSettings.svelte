<script lang="ts">
	import { Download, ExternalLink } from 'lucide-svelte';
	import { settingsStore } from '$lib/stores/settingsStore.svelte';
	import { openUrl } from '@tauri-apps/plugin-opener';
</script>

<section class="settings-section glass-panel">
	<div class="s-header">
		<ExternalLink size={18} class="accent-icon" />
		<h3>Release Channel</h3>
	</div>
	<div class="s-body">
		<div class="data-item">
			<span class="d-label">Core Engine Version</span>
			<span class="d-val">v{settingsStore.appVersion}</span>
		</div>
		<div class="data-item">
			<span class="d-label">Update Method</span>
			<span class="d-val">Manual unsigned installer</span>
		</div>
		<p class="section-desc">
			Automatic updater signing is disabled. Download the latest installer and choose the CUDA or
			DirectML Windows build that matches your GPU; ordinary reinstalls preserve application data.
		</p>
	</div>
	<footer class="s-footer">
		<button class="s-btn primary" onclick={() => openUrl('https://github.com/KodyDennon/pursue/releases/latest')}>
			<Download size={14} /> Download Latest Release
		</button>
		<button class="s-btn secondary" onclick={() => openUrl('https://downloads.kodydennon.com/releases/latest/manifest.json')}>
			<ExternalLink size={14} /> R2 Mirror Manifest
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
		display: flex;
		gap: var(--space-lg);
		flex-wrap: wrap;
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

	.s-btn.primary {
		background: var(--color-accent-primary);
		color: #000;
	}

	.s-btn:hover {
		filter: brightness(1.1);
		transform: translateY(-1px);
	}

	.s-btn:disabled {
		opacity: 0.55;
		cursor: wait;
		transform: none;
	}

	.s-btn.secondary {
		background: transparent;
		border-color: var(--color-border-subtle);
		color: var(--color-text-primary);
	}

	:global(.accent-icon) {
		color: var(--color-accent-primary);
	}
</style>
