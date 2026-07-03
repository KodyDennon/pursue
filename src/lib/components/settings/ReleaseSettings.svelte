<script lang="ts">
	import { ExternalLink } from 'lucide-svelte';
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
		<p class="section-desc">
			Automatic updates are disabled until installer signing, notarization, and artifact trust are
			complete. Install production releases manually from GitHub.
		</p>
	</div>
	<footer class="s-footer">
		<button
			class="s-btn primary"
			onclick={() => openUrl('https://github.com/KodyDennon/pursue/releases')}
		>
			<ExternalLink size={14} />
			Open GitHub Releases
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
		padding: var(--space-5xl);
		display: flex;
		align-items: center;
		gap: var(--space-3xl);
		border-bottom: 1px solid var(--border-subtle);
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
		color: var(--text-secondary);
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
		color: var(--text-tertiary);
	}

	.d-val {
		font-size: var(--text-md);
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
		background: var(--accent-primary);
		color: #000;
	}

	.s-btn:hover {
		filter: brightness(1.1);
		transform: translateY(-1px);
	}

	:global(.accent-icon) {
		color: var(--accent-primary);
	}
</style>
