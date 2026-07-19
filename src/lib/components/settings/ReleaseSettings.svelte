<script lang="ts">
	import { Download, ExternalLink, RefreshCcw } from 'lucide-svelte';
	import { settingsStore } from '$lib/stores/settingsStore.svelte';
	import { updateStore } from '$lib/stores/updateStore.svelte';
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
			<span class="d-label">Signed Update Lane</span>
			<span class="d-val">{updateStore.target}</span>
		</div>
		{#if updateStore.phase === 'available'}
			<p class="section-desc update-ready">
				Signed update v{updateStore.availableVersion} is ready to download.
			</p>
		{:else if updateStore.phase === 'downloading'}
			<p class="section-desc">Downloading and verifying: {updateStore.progressPercent}%</p>
		{:else if updateStore.phase === 'installing'}
			<p class="section-desc">Installing the verified update. PURSUE will restart shortly.</p>
		{:else if updateStore.phase === 'current'}
			<p class="section-desc">This signed release lane is current.</p>
		{:else if updateStore.phase === 'error'}
			<p class="section-desc update-error">Update check failed safely: {updateStore.error}</p>
		{:else}
			<p class="section-desc">
				Updates are signature-verified and remain on the same GPU provider lane.
			</p>
		{/if}
	</div>
	<footer class="s-footer">
		{#if updateStore.phase === 'available'}
			<button class="s-btn primary" onclick={() => updateStore.downloadAndInstall()}>
				<Download size={14} />
				Download and Install v{updateStore.availableVersion}
			</button>
		{:else}
			<button
				class="s-btn primary"
				disabled={updateStore.phase === 'checking' || updateStore.phase === 'downloading' || updateStore.phase === 'installing'}
				onclick={() => updateStore.checkForUpdate()}
			>
				<RefreshCcw size={14} />
				{updateStore.phase === 'checking' ? 'Checking...' : 'Check for Updates'}
			</button>
		{/if}
		<button class="s-btn secondary" onclick={() => openUrl('https://github.com/KodyDennon/pursue/releases')}>
			<ExternalLink size={14} /> Manual Download
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

	.update-ready {
		color: var(--color-accent-primary);
	}

	.update-error {
		color: var(--color-status-error, #ff6b6b);
	}

	:global(.accent-icon) {
		color: var(--color-accent-primary);
	}
</style>
