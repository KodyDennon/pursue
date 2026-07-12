import { invoke } from '@tauri-apps/api/core';
import { getVersion } from '@tauri-apps/api/app';
import { listen } from '@tauri-apps/api/event';
import { open } from '@tauri-apps/plugin-dialog';
import { addToast } from '$lib/stores/toastStore.svelte';
import { logger } from '$lib/logger';
import type { DatabaseStatus, StorageLocationInfo, StorageMigrationProgress } from '$lib/types';

type AgentSettings = {
	auto_sync: boolean;
	auto_analyze: boolean;
};

class SettingsStore {
	status = $state<DatabaseStatus | null>(null);
	busy = $state<string | null>(null);
	agentSettings = $state<AgentSettings>({ auto_sync: true, auto_analyze: false });
	performanceMode = $state(false);
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
	storageLocation = $state<StorageLocationInfo | null>(null);
	migration = $state<StorageMigrationProgress | null>(null);
	private storageFallbackNotified = false;

	async init() {
		await Promise.all([
			this.loadStatus(),
			this.loadAppSettings(),
			this.loadVersion(),
			this.loadStorageLocation()
		]);
	}

	async loadStorageLocation() {
		try {
			this.storageLocation = await invoke<StorageLocationInfo>('get_storage_location');
			if (this.storageLocation.last_migration_error) {
				addToast({
					type: 'error',
					message: `Storage migration failed: ${this.storageLocation.last_migration_error}. Your data is still at the previous location.`,
					duration: 0
				});
			}
			if (this.storageLocation.is_fallback && !this.storageFallbackNotified) {
				this.storageFallbackNotified = true;
				addToast({
					type: 'error',
					message: `Configured storage location ${this.storageLocation.configured_root} is unreachable. Using the default location for this session.`,
					duration: 0
				});
			}
		} catch (e) {
			logger.error('Failed to load storage location:', e);
		}
	}

	async changeStorageLocation(newRoot?: string) {
		const target =
			newRoot ??
			(await open({
				directory: true,
				title: 'Choose a folder for PURSUE storage'
			}));
		if (!target || Array.isArray(target)) return;

		const size = this.status
			? ` (~${Math.max(1, Math.round((this.status.artifact_bytes + this.status.database_bytes) / 1024 / 1024))} MB plus models)`
			: '';
		if (
			!confirm(
				`Route all PURSUE storage to:\n${target}\n\nExisting data${size} will be copied there and the application will restart. Files at the old location are left in place; you can delete them after verifying the move.\n\nPROCEED?`
			)
		)
			return;

		this.busy = 'storage';
		this.migration = { status: 'copying', bytes_copied: 0, bytes_total: 0 };
		const unlisten = await listen<StorageMigrationProgress>(
			'storage-migration-progress',
			(event) => {
				this.migration = event.payload;
				if (event.payload.status === 'error') {
					addToast({
						type: 'error',
						message: `Storage migration failed: ${event.payload.message ?? 'unknown error'}. Restarting on the previous location.`,
						duration: 0
					});
				}
			}
		);
		try {
			// Does not resolve on success: the backend restarts the app.
			await invoke('set_storage_location', { newRoot: target, migrate: true });
		} catch (e) {
			this.migration = null;
			addToast({ type: 'error', message: `Storage location change failed: ${e}` });
		} finally {
			unlisten();
			this.busy = null;
		}
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
			const s = await invoke<AgentSettings | null>('get_app_settings', { key: 'ingestion_agent' });
			if (s) this.agentSettings = s;

			const p = await invoke<string>('get_app_settings', { key: 'intelligence_persona' });
			if (typeof p === 'string') this.personaModifier = p;

			const t = await invoke<string>('get_app_settings', { key: 'huggingface_token' });
			if (typeof t === 'string') this.hfToken = t;

			const perf = await invoke<boolean | null>('get_app_settings', { key: 'performance_mode' });
			if (typeof perf === 'boolean') this.performanceMode = perf;
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

	async savePerformanceMode() {
		try {
			await invoke('set_app_settings', { key: 'performance_mode', value: this.performanceMode });
			addToast({ type: 'success', message: 'Performance Mode Updated', duration: 2000 });
		} catch (e) {
			addToast({ type: 'error', message: `Failed to save performance mode: ${e}` });
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
			await invoke('set_app_settings', {
				key: 'intelligence_persona',
				value: this.personaModifier
			});
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
				message: `Evidence cache cleared: ${report.files_removed} files removed.`,
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
