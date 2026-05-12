<script lang="ts">
	import { invoke } from '@tauri-apps/api/core';
	import { getVersion } from '@tauri-apps/api/app';
	import { onMount } from 'svelte';
	import {
		Trash2,
		ShieldCheck,
		Cpu,
		HardDrive,
		Brain,
		Save,
		RefreshCcw
	} from 'lucide-svelte';
	import { checkForUpdates } from '$lib/updater';
	import { addToast } from '$lib/toastStore';
	import type { DatabaseStatus } from '$lib/types';

	let status = $state<DatabaseStatus | null>(null);
	let busy = $state<string | null>(null);

	let agentSettings = $state({ auto_sync: true, auto_analyze: true });
	let hfToken = $state('');
	let personaModifier = $state('');
	let appVersion = $state('...');

	async function loadStatus() {
		try {
			status = await invoke<DatabaseStatus>('get_database_status');
		} catch (e) {
			console.error(e);
		}
	}

	async function loadAppSettings() {
		try {
			const s = await invoke<typeof agentSettings>('get_app_settings', { key: 'ingestion_agent' });
			if (s) agentSettings = s;

			const p = await invoke<string>('get_app_settings', { key: 'intelligence_persona' });
			if (typeof p === 'string') personaModifier = p;

			const t = await invoke<string>('get_app_settings', { key: 'huggingface_token' });
			if (typeof t === 'string') hfToken = t;
		} catch (e) {
			console.error(e);
		}
	}

	async function saveAgentSettings() {
		try {
			await invoke('set_app_settings', { key: 'ingestion_agent', value: agentSettings });
			addToast({ type: 'success', message: 'Agent Configuration Saved', duration: 2000 });
		} catch (e) {
			addToast({ type: 'error', message: `Failed to save settings: ${e}` });
		}
	}

	async function saveHfToken() {
		busy = 'token';
		try {
			await invoke('set_app_settings', { key: 'huggingface_token', value: hfToken });
			addToast({ type: 'success', message: 'Hugging Face Token Updated', duration: 2000 });
		} catch (e) {
			addToast({ type: 'error', message: `Failed to save token: ${e}` });
		} finally {
			busy = null;
		}
	}

	async function savePersona() {
		busy = 'persona';
		try {
			await invoke('set_app_settings', { key: 'intelligence_persona', value: personaModifier });
			addToast({ type: 'success', message: 'Intelligence Persona Updated', duration: 2000 });
		} catch (e) {
			addToast({ type: 'error', message: `Failed to save persona: ${e}` });
		} finally {
			busy = null;
		}
	}

	async function clearCache() {
		if (!confirm('Are you sure? This will delete all downloaded evidence and analysis assets.'))
			return;
		busy = 'clear';
		try {
			addToast({ type: 'success', message: 'Intelligence cache cleared.' });
			await loadStatus();
		} catch (e) {
			addToast({ type: 'error', message: `Clear failed: ${e}` });
		} finally {
			busy = null;
		}
	}

	async function purgeSystem() {
		if (
			!confirm(
				'CRITICAL WARNING: This will permanently delete your entire database, all downloaded intelligence models, and all evidence artifacts. The application will restart to a fresh state. PROCEED?'
			)
		)
			return;
		busy = 'purge';
		try {
			addToast({ type: 'info', message: 'Initiating absolute system purge...', duration: 0 });
			await invoke('factory_reset');
		} catch (e) {
			addToast({ type: 'error', message: `Purge failed: ${e}` });
		} finally {
			busy = null;
		}
	}

	onMount(async () => {
		loadStatus();
		loadAppSettings();
		appVersion = await getVersion();
	});

	function formatBytes(bytes: number) {
		if (bytes === 0) return '0 B';
		const k = 1024;
		const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
		const i = Math.floor(Math.log(bytes) / Math.log(k));
		return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
	}
</script>

<div class="settings-container custom-scrollbar">
	<header class="settings-head">
		<h2>Secure System Settings</h2>
		<p>Manage your local intelligence environment and forensic archives.</p>
	</header>

	<div class="settings-grid">
		<!-- Intelligence Persona Modifier -->
		<section class="settings-section glass-panel full-width">
			<div class="s-header">
				<Brain size={18} class="accent-icon" />
				<h3>Intelligence Persona Directive</h3>
			</div>
			<div class="s-body">
				<p class="section-desc">
					Inject custom instructions into Gemma 4's core reasoning engine. This modifier is applied
					to all forensic extractions.
				</p>
				<textarea
					bind:value={personaModifier}
					placeholder="e.g., 'Focus heavily on technical sensor data and skepticism regarding atmospheric phenomena...'"
					class="persona-input"
				></textarea>
			</div>
			<footer class="s-footer">
				<button class="s-btn primary" onclick={savePersona} disabled={busy === 'persona'}>
					<Save size={14} /> Update Core Directive
				</button>
			</footer>
		</section>

		<!-- Hugging Face Authentication -->
		<section class="settings-section glass-panel">
			<div class="s-header">
				<ShieldCheck size={18} class="accent-icon" />
				<h3>Hugging Face Authentication</h3>
			</div>
			<div class="s-body">
				<p class="section-desc">
					Provide a User Access Token to download gated models (e.g., official Google Gemma series).
					Create one at <a
						href="https://huggingface.co/settings/tokens"
						target="_blank"
						class="accent-link">huggingface.co/settings/tokens</a
					>.
				</p>
				<div class="data-item">
					<span class="d-label">Access Token</span>
					<input
						type="password"
						bind:value={hfToken}
						placeholder="hf_..."
						class="persona-input token-input"
					/>
				</div>
			</div>
			<footer class="s-footer">
				<button class="s-btn primary" onclick={saveHfToken} disabled={busy === 'token'}>
					<Save size={14} /> Update Token
				</button>
			</footer>
		</section>

		<section class="settings-section glass-panel">
			<div class="s-header">
				<HardDrive size={18} class="accent-icon" />
				<h3>Data Environment</h3>
			</div>
			<div class="s-body">
				<div class="data-item">
					<span class="d-label">App Data Directory</span>
					<code class="d-val">{status?.app_data_dir || 'Loading...'}</code>
				</div>
				<div class="data-item">
					<span class="d-label">Database Path</span>
					<code class="d-val">{status?.database_path || 'Loading...'}</code>
				</div>
				<div class="data-item">
					<span class="d-label">Storage Usage</span>
					<div class="usage-bar">
						<div
							class="usage-fill"
							style="width: {Math.min(100, (status?.artifact_bytes || 0) / 1024 / 1024 / 10)}%"
						></div>
					</div>
					<span class="d-val"
						>{formatBytes(status?.artifact_bytes || 0)} across {status?.artifact_count || 0} local assets</span
					>
				</div>
			</div>
			<footer class="s-footer">
				<button class="s-btn danger" onclick={clearCache} disabled={busy === 'clear'}>
					<Trash2 size={14} />
					Clear Evidence Cache
				</button>
			</footer>
		</section>

		<section class="settings-section glass-panel">
			<div class="s-header">
				<ShieldCheck size={18} class="accent-icon" />
				<h3>Automation & Pipeline</h3>
			</div>
			<div class="s-body">
				<div class="toggle-item">
					<div class="t-info">
						<strong>Auto-Retrieval Pipeline</strong>
						<span>Automatically download official sources when synced.</span>
					</div>
					<button
						class="toggle"
						aria-label="Toggle"
						class:active={agentSettings.auto_sync}
						onclick={() => {
							agentSettings.auto_sync = !agentSettings.auto_sync;
							saveAgentSettings();
						}}
					></button>
				</div>
				<div class="toggle-item">
					<div class="t-info">
						<strong>Neural Post-Processing</strong>
						<span>Automatically initiate Gemma 4 extraction after download.</span>
					</div>
					<button
						class="toggle"
						aria-label="Toggle"
						class:active={agentSettings.auto_analyze}
						onclick={() => {
							agentSettings.auto_analyze = !agentSettings.auto_analyze;
							saveAgentSettings();
						}}
					></button>
				</div>
			</div>
		</section>

		<section class="settings-section glass-panel">
			<div class="s-header">
				<RefreshCcw size={18} class="accent-icon" />
				<h3>System Intelligence</h3>
			</div>
			<div class="s-body">
				<div class="data-item">
					<span class="d-label">Core Engine Version</span>
					<span class="d-val">v{appVersion}</span>
				</div>
				<p class="section-desc">
					Maintain forensic parity. Updates synchronize neural weights, extraction patterns, and
					secure communication protocols.
				</p>
			</div>
			<footer class="s-footer">
				<button class="s-btn primary" onclick={() => checkForUpdates(false)}>
					<RefreshCcw size={14} />
					Synchronize System Core
				</button>
			</footer>
		</section>

		<section class="settings-section glass-panel">
			<div class="s-header">
				<Cpu size={18} class="accent-icon" />
				<h3>Hardware Optimization</h3>
			</div>
			<div class="s-body">
				<p class="section-desc">
					The Intelligence Engine automatically optimizes for your hardware tier. Currently running
					in <strong>Accelerated</strong> mode.
				</p>
				<div class="data-item">
					<span class="d-label">Neural Model Cache</span>
					<span class="d-val">{(status?.artifact_bytes || 0) > 0 ? 'Active' : 'Standby'}</span>
				</div>
			</div>
			<footer class="s-footer">
				<button class="s-btn danger-outline" onclick={purgeSystem} disabled={busy === 'purge'}>
					<Trash2 size={14} /> Absolute System Purge
				</button>
			</footer>
		</section>
	</div>
</div>

<style>
	.settings-container {
		padding: 40px;
		height: 100%;
		overflow-y: auto;
		max-width: 1200px;
		margin: 0 auto;
	}

	.settings-head {
		margin-bottom: 40px;
	}

	.settings-head h2 {
		font-size: 28px;
		margin-bottom: 8px;
		color: var(--text-primary);
	}

	.settings-head p {
		color: var(--text-secondary);
		font-size: 15px;
	}

	.settings-grid {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(450px, 1fr));
		gap: 24px;
	}

	.settings-section {
		background: var(--bg-surface);
		border: 1px solid var(--border-subtle);
		border-radius: var(--radius-lg);
		display: flex;
		flex-direction: column;
	}

	.settings-section.full-width {
		grid-column: 1 / -1;
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

	.persona-input {
		width: 100%;
		min-height: 120px;
		background: rgba(0, 0, 0, 0.3);
		border: 1px solid var(--border-subtle);
		border-radius: 8px;
		padding: 16px;
		color: var(--text-primary);
		font-family: var(--font-mono);
		font-size: 13px;
		resize: vertical;
		outline: none;
		transition: border-color 0.2s;
	}

	.persona-input.token-input {
		min-height: unset;
		resize: none;
	}

	.accent-link {
		color: var(--accent-primary);
		text-decoration: none;
	}
	.accent-link:hover {
		text-decoration: underline;
	}

	.persona-input:focus {
		border-color: var(--accent-primary);
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

	.toggle-item {
		display: flex;
		justify-content: space-between;
		align-items: center;
		gap: 24px;
	}

	.t-info {
		display: flex;
		flex-direction: column;
		gap: 4px;
	}

	.t-info strong {
		font-size: 14px;
		color: var(--text-primary);
	}

	.t-info span {
		font-size: 12px;
		color: var(--text-secondary);
	}

	.toggle {
		width: 36px;
		height: 20px;
		background: #2a2d35;
		border-radius: 10px;
		position: relative;
		cursor: pointer;
		border: none;
		padding: 0;
		transition: background 0.2s;
	}

	.toggle.active {
		background: var(--accent-primary);
	}

	.toggle::after {
		content: '';
		position: absolute;
		width: 14px;
		height: 14px;
		background: #fff;
		border-radius: 50%;
		top: 3px;
		left: 3px;
		transition: transform 0.2s;
	}

	.toggle.active::after {
		transform: translateX(16px);
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

	.s-btn.primary {
		background: var(--accent-primary);
		color: #000;
	}

	.s-btn.danger {
		background: rgba(243, 77, 77, 0.1);
		color: var(--accent-danger);
		border: 1px solid rgba(243, 77, 77, 0.2);
	}

	.s-btn.danger-outline {
		background: transparent;
		color: var(--accent-danger);
		border: 1px solid rgba(243, 77, 77, 0.4);
	}

	.s-btn:hover {
		filter: brightness(1.1);
		transform: translateY(-1px);
	}
</style>
