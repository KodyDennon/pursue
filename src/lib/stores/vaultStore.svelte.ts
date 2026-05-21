import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { addToast } from '$lib/toastStore';
import { logger } from '$lib/logger';

class VaultStore {
	busy = $state(false);
	verifyProgress = $state(0);
	verifyStatusText = $state('');
	encryptionStatus = $state<{ enabled: boolean; algorithm: string } | null>(null);

	async init() {
		try {
			this.encryptionStatus = await invoke('get_vault_encryption_status');
		} catch (e) {
			logger.error('Failed to load encryption status:', e);
		}
	}

	async runIntegrityCheck() {
		if (this.busy) return;
		this.busy = true;
		this.verifyProgress = 0;
		this.verifyStatusText = 'Initiating SHA-256 integrity sweep across vault...';
		addToast({ type: 'info', message: this.verifyStatusText, duration: 3000 });

		let unlisten: (() => void) | null = null;
		try {
			unlisten = await listen<{ current: number; total: number; status: string }>(
				'integrity-progress',
				(event) => {
					const { current, total, status } = event.payload;
					if (total > 0) {
						this.verifyProgress = (current / total) * 100;
					}
					if (status === 'completed') {
						this.verifyStatusText = 'Finalizing report...';
					} else {
						this.verifyStatusText = `Verifying artifact ${current} of ${total}...`;
					}
				}
			);

			const report = await invoke<{ verified: number; corrupted: number; missing: number }>(
				'verify_vault_integrity'
			);

			if (report.corrupted === 0 && report.missing === 0) {
				addToast({
					type: 'success',
					message: `Integrity check complete. All ${report.verified} local artifacts verified.`,
					duration: 5000
				});
			} else {
				addToast({
					type: 'error',
					message: `Integrity failure: ${report.corrupted} corrupted, ${report.missing} missing.`,
					duration: 8000
				});
			}
		} catch (e) {
			addToast({ type: 'error', message: `Integrity check failed: ${e}`, duration: 5000 });
		} finally {
			if (unlisten) unlisten();
			this.busy = false;
			this.verifyStatusText = '';
			this.verifyProgress = 0;
		}
	}
}

export const vaultStore = new VaultStore();
