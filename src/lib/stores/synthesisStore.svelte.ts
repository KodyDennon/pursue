import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { logger } from '$lib/logger';

export interface AnalysisProgress {
	status: string;
	current?: number;
	total?: number;
	record_id?: string;
	error?: string;
	token_text?: string;
	token_index?: number;
	progress?: number;
	msg?: string;
	telemetry?: {
		input_shape: number[];
		kv_cache_shape: number[];
		device: string;
	};
}

class SynthesisStore {
	status = $state('standby');
	currentRecordId = $state<string | null>(null);
	neuralTelemetry = $state<AnalysisProgress['telemetry'] | null>(null);
	thoughtText = $state('');
	busy = $state(false);

	currentBatchIndex = $state(0);
	totalBatchCount = $state(0);

	modelDownloadProgress = $state(0);
	modelDownloadMsg = $state('');

	private unlisten: UnlistenFn | null = null;

	async init(isOpen: boolean, setOpen: (open: boolean) => void, onComplete?: () => void) {
		logger.debug('[SynthesisStore] Initializing neural synthesis events...');

		this.unlisten = await listen<AnalysisProgress>('analysis-progress', (event) => {
			const payload = event.payload;

			const intelligenceStatuses = ['loading-model', 'synthesizing-start', 'synthesizing'];

			if (intelligenceStatuses.includes(payload.status)) {
				if (!isOpen) {
					setOpen(true);
				}
				this.busy = true;
				this.status = payload.status === 'synthesizing-start' ? 'synthesizing' : payload.status;
			} else if (payload.status === 'completed') {
				if (this.busy) {
					this.status = 'completed';
					this.busy = false;
					if (onComplete) onComplete();
				}
				return;
			} else if (payload.status === 'failed') {
				if (this.busy) {
					this.status = 'failed';
					this.busy = false;
				}
				return;
			} else {
				return;
			}

			this.currentRecordId = payload.record_id ?? this.currentRecordId;
			if (payload.current !== undefined) {
				this.currentBatchIndex = payload.current;
			}
			if (payload.total !== undefined) {
				this.totalBatchCount = payload.total;
			}

			if (payload.status === 'loading-model') {
				this.modelDownloadProgress = payload.progress ?? this.modelDownloadProgress;
				this.modelDownloadMsg = payload.msg ?? this.modelDownloadMsg;
			} else if (payload.status === 'synthesizing-start') {
				this.thoughtText = ''; // Reset for new synthesis
				this.neuralTelemetry = null;
			} else if (payload.status === 'synthesizing') {
				if (payload.token_text) {
					this.thoughtText += payload.token_text;
				}
				if (payload.telemetry) {
					this.neuralTelemetry = payload.telemetry;
				}
			}
		});
	}

	destroy() {
		if (this.unlisten) this.unlisten();
	}
}

export const synthesisStore = new SynthesisStore();
