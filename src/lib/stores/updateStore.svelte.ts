import { invoke } from '@tauri-apps/api/core';
import { check, type Update } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';
import { addToast, updateToast } from '$lib/stores/toastStore.svelte';
import { logger } from '$lib/logger';

export type UpdatePhase =
	| 'idle'
	| 'checking'
	| 'current'
	| 'available'
	| 'downloading'
	| 'installing'
	| 'error';

class UpdateStore {
	phase = $state<UpdatePhase>('idle');
	availableVersion = $state<string | null>(null);
	target = $state<string>('detecting');
	downloadedBytes = $state(0);
	totalBytes = $state(0);
	error = $state<string | null>(null);
	private pendingUpdate: Update | null = null;
	private automaticCheckStarted = false;

	get progressPercent(): number {
		if (this.totalBytes <= 0) return 0;
		return Math.min(100, Math.round((this.downloadedBytes / this.totalBytes) * 100));
	}

	async checkForUpdate(options: { silent?: boolean; automatic?: boolean } = {}) {
		if (options.automatic) {
			if (this.automaticCheckStarted) return;
			this.automaticCheckStarted = true;
		}
		if (this.phase === 'checking' || this.phase === 'downloading' || this.phase === 'installing') {
			return;
		}

		this.phase = 'checking';
		this.error = null;
		try {
			this.target = await invoke<string>('get_update_target');
			if (this.target === 'unsupported-development-build') {
				this.phase = 'current';
				if (!options.silent) {
					addToast({ type: 'info', message: 'Updates are only available in production builds.' });
				}
				return;
			}

			this.pendingUpdate = await check({ target: this.target, timeout: 30_000 });
			if (!this.pendingUpdate) {
				this.availableVersion = null;
				this.phase = 'current';
				if (!options.silent) {
					addToast({ type: 'success', message: 'PURSUE is up to date.', duration: 2500 });
				}
				return;
			}

			this.availableVersion = this.pendingUpdate.version;
			this.phase = 'available';
			addToast({
				type: 'info',
				message: `PURSUE v${this.pendingUpdate.version} is available for ${this.target}. Open Settings to install it.`,
				duration: options.automatic ? 8000 : 4000
			});
		} catch (cause) {
			this.fail(cause, !options.silent);
		}
	}

	async downloadAndInstall() {
		if (!this.pendingUpdate || this.phase !== 'available') {
			await this.checkForUpdate();
			if (!this.pendingUpdate || this.phase !== 'available') return;
		}

		this.phase = 'downloading';
		this.downloadedBytes = 0;
		this.totalBytes = 0;
		this.error = null;
		const toastId = addToast({
			type: 'loading',
			message: `Downloading signed PURSUE v${this.pendingUpdate.version} update...`,
			duration: 0
		});

		try {
			await this.pendingUpdate.download((event) => {
				switch (event.event) {
					case 'Started':
						this.totalBytes = event.data.contentLength ?? 0;
						break;
					case 'Progress':
						this.downloadedBytes += event.data.chunkLength;
						updateToast(toastId, {
							message: `Downloading signed update... ${this.progressPercent}%`
						});
						break;
					case 'Finished':
						this.phase = 'installing';
						break;
				}
			});
			updateToast(toastId, { message: 'Signature verified. Preparing durable storage...' });
			await invoke('prepare_for_update');
			updateToast(toastId, { message: 'Installing verified update...' });
			await this.pendingUpdate.install();

			updateToast(toastId, {
				type: 'success',
				message: 'Update installed. Restarting PURSUE...',
				duration: 2500
			});
			await relaunch();
		} catch (cause) {
			this.fail(cause, false);
			updateToast(toastId, {
				type: 'error',
				message: `Update failed safely: ${this.error}`,
				duration: 0
			});
		}
	}

	private fail(cause: unknown, notify: boolean) {
		this.error = cause instanceof Error ? cause.message : String(cause);
		this.phase = 'error';
		logger.error('Signed update operation failed:', cause);
		if (notify) {
			addToast({
				type: 'error',
				message: `Update check failed: ${this.error}`,
				duration: 0
			});
		}
	}
}

export const updateStore = new UpdateStore();
