import { invoke } from '@tauri-apps/api/core';
import { getVersion } from '@tauri-apps/api/app';
import { addToast } from '$lib/toastStore';
import { logger } from '$lib/logger';
import type { DatabaseStatus } from '$lib/types';

class SettingsStore {
	status = $state<DatabaseStatus | null>(null);
	busy = $state<string | null>(null);
	agentSettings = $state({ auto_sync: true, auto_analyze: true });
	hfToken = $state('');
	personaModifier = $state('');
	appVersion = $state('...');
	encryptionStatus = $state<{
		enabled: boolean;
		algorithm: string;
		encrypted_artifacts: boolean;
		encrypted_exports: boolean;
		integrity_layer: string;
	} | null>(null);

	async init() {
		await Promise.all([this.loadStatus(), this.loadAppSettings(), this.loadVersion()]);
	}

	async loadVersion() {
		this.appVersion = await getVersion();
	}

	async loadStatus() {
		try {
			this.status = await invoke<DatabaseStatus>('get_database_status');
			this.encryptionStatus = await invoke('get_vault_encryption_status');
		} catch (e) {
			logger.error('Failed to load status:', e);
		}
	}

	async loadAppSettings() {
		try {
			const s = await invoke<any>('get_app_settings', { key: 'ingestion_agent' });
			if (s) this.agentSettings = s;

			const p = await invoke<string>('get_app_settings', { key: 'intelligence_persona' });
			if (typeof p === 'string') this.personaModifier = p;

			const t = await invoke<string>('get_app_settings', { key: 'huggingface_token' });
			if (typeof t === 'string') this.hfToken = t;
		} catch (e) {
			logger.error('Failed to load app settings:', e);
		}
	}

	async saveAgentSettings() {
		try {
			await invoke('set_app_settings', { key: 'ingestion_agent', value: this.agentSettings });
			addToast({ type: 'success', message: 'Agent Configuration Saved', duration: 2000 });
		} catch (e) {
			addToast({ type: 'error', message: `Failed to save settings: ${e}` });
		}
	}

	async saveHfToken() {
		this.busy = 'token';
		try {
			await invoke('set_app_settings', { key: 'huggingface_token', value: this.hfToken });
			addToast({ type: 'success', message: 'Hugging Face Token Updated', duration: 2000 });
		} catch (e) {
			addToast({ type: 'error', message: `Failed to save token: ${e}` });
		} finally {
			this.busy = null;
		}
	}

	async savePersona() {
		this.busy = 'persona';
		try {
			await invoke('set_app_settings', { key: 'intelligence_persona', value: this.personaModifier });
			addToast({ type: 'success', message: 'Intelligence Persona Updated', duration: 2000 });
		} catch (e) {
			addToast({ type: 'error', message: `Failed to save persona: ${e}` });
		} finally {
			this.busy = null;
		}
	}

	async clearCache() {
		if (!confirm('Are you sure? This will delete all downloaded evidence and analysis assets.'))
			return;
		this.busy = 'clear';
		try {
			const report = await invoke<{ files_removed: number; bytes_removed: number }>(
				'clear_evidence_cache'
			);
			addToast({
				type: 'success',
				message: `Evidence cache cleared.`,
				duration: 4000
			});
			await this.loadStatus();
		} catch (e) {
			addToast({ type: 'error', message: `Clear failed: ${e}` });
		} finally {
			this.busy = null;
		}
	}

	async purgeSystem() {
		if (
			!confirm(
				'CRITICAL WARNING: This will permanently delete your entire database, all downloaded intelligence models, and all evidence artifacts. The application will restart to a fresh state. PROCEED?'
			)
		)
			return;
		this.busy = 'purge';
		try {
			addToast({ type: 'info', message: 'Initiating absolute system purge...', duration: 0 });
			await invoke('factory_reset');
		} catch (e) {
			addToast({ type: 'error', message: `Purge failed: ${e}` });
		} finally {
			this.busy = null;
		}
	}
}

export const settingsStore = new SettingsStore();
