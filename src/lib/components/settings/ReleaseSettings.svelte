<script lang="ts">
	import { Download, ExternalLink, RefreshCw, CheckCircle2, ArrowUpCircle } from 'lucide-svelte';
	import { settingsStore } from '$lib/stores/settingsStore.svelte';
	import { updateStore } from '$lib/stores/updateStore.svelte';
	import { openUrl } from '@tauri-apps/plugin-opener';
</script>

<section class="settings-section glass-panel">
	<div class="s-header">
		<ExternalLink size={18} class="accent-icon" />
		<h3>Release Channel & Auto-Updater</h3>
	</div>
	<div class="s-body">
		<div class="data-item">
			<span class="d-label">Core Engine Version</span>
			<div class="version-row">
				<span class="d-val">v{settingsStore.appVersion}</span>
				{#if updateStore.phase === 'available'}
					<span class="badge update-available"><ArrowUpCircle size={14} /> v{updateStore.availableVersion} Available</span>
				{:else if updateStore.phase === 'current'}
					<span class="badge up-to-date"><CheckCircle2 size={14} /> Up to Date</span>
				{:else if updateStore.phase === 'checking'}
					<span class="badge checking"><RefreshCw size={14} class="spin" /> Checking...</span>
				{:else if updateStore.phase === 'downloading'}
					<span class="badge downloading">Downloading ({updateStore.progressPercent}%)</span>
				{/if}
			</div>
		</div>

		<div class="data-item">
			<span class="d-label">Target Architecture</span>
			<span class="d-val">{updateStore.target || 'detecting...'}</span>
		</div>

		<p class="section-desc">
			PURSUE Data Analyzer automatically checks for new production releases on launch via the Cloudflare R2 mirror and GitHub Releases portal. Reinstalls preserve all local vector embeddings and case databases.
		</p>
	</div>
	<footer class="s-footer">
		<button
			class="s-btn secondary"
			disabled={updateStore.phase === 'checking' || updateStore.phase === 'downloading'}
			onclick={() => updateStore.checkForUpdate()}
		>
			<RefreshCw size={14} class={updateStore.phase === 'checking' ? 'spin' : ''} />
			Check for Updates
		</button>

		{#if updateStore.phase === 'available'}
			<button
				class="s-btn primary"
				onclick={() => updateStore.downloadAndInstall()}
			>
				<ArrowUpCircle size={14} /> Install Update v{updateStore.availableVersion}
			</button>
		{:else}
			<button
				class="s-btn primary"
				onclick={() => openUrl('https://downloads.kodydennon.com')}
			>
				<Download size={14} /> Downloads Portal
			</button>
		{/if}

		<button
			class="s-btn secondary"
			onclick={() => openUrl('https://github.com/KodyDennon/pursue/releases/latest')}
		>
			<ExternalLink size={14} /> GitHub Releases
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

	.version-row {
		display: flex;
		align-items: center;
		gap: 12px;
	}

	.badge {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		padding: 4px 10px;
		border-radius: var(--radius-sm);
		font-size: var(--text-xs);
		font-weight: 600;
	}

	.badge.up-to-date {
		background: rgba(34, 197, 94, 0.15);
		color: #4ade80;
		border: 1px solid rgba(34, 197, 94, 0.3);
	}

	.badge.update-available {
		background: rgba(59, 130, 246, 0.2);
		color: #60a5fa;
		border: 1px solid rgba(59, 130, 246, 0.4);
	}

	.badge.checking, .badge.downloading {
		background: rgba(234, 179, 8, 0.15);
		color: #facc15;
		border: 1px solid rgba(234, 179, 8, 0.3);
	}

	:global(.spin) {
		animation: spin 1s linear infinite;
	}

	@keyframes spin {
		from { transform: rotate(0deg); }
		to { transform: rotate(360deg); }
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
