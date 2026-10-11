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
	token_limit?: number;
	progress?: number;
	msg?: string;
	telemetry?: {
		input_shape?: number[];
		kv_cache_shape?: number[];
		device?: string;
		kv_cache?: string;
		gpu_layers?: number;
		context_size?: number;
		visual_asset_count?: number;
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
	batchStartedAt = $state<number | null>(null);
	recordStartedAt = $state<number | null>(null);
	/** Durations of records that already finished in this batch, in milliseconds. */
	completedRecordMs = $state<number[]>([]);
	tokenIndex = $state(0);
	tokenLimit = $state(0);

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
					this.noteRecordFinished();
					this.status = 'completed';
					this.busy = false;
					if (onComplete) onComplete();
				}
				return;
			} else if (payload.status === 'failed') {
				if (this.busy) {
					this.noteRecordFinished();
					this.status = 'failed';
					this.busy = false;
				}
				return;
			} else if (payload.status === 'record-failed') {
				this.noteRecordFinished();
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
				if (payload.current === 1) {
					this.recordStartedAt = null;
					this.completedRecordMs = [];
					this.batchStartedAt = null;
				}
				this.noteRecordFinished();
				const started = Date.now();
				if (this.batchStartedAt === null) this.batchStartedAt = started;
				this.recordStartedAt = started;
				this.tokenIndex = 0;
				this.tokenLimit = 0;
				this.thoughtText = '';
				this.neuralTelemetry = null;
			} else if (payload.status === 'synthesizing') {
				if (payload.token_index !== undefined) this.tokenIndex = payload.token_index;
				if (payload.token_limit !== undefined) this.tokenLimit = payload.token_limit;
				if (payload.token_text) {
					this.thoughtText += payload.token_text;
				}
				if (payload.telemetry) {
					this.neuralTelemetry = payload.telemetry;
				}
			}
		});
	}

	private noteRecordFinished() {
		if (this.recordStartedAt === null) return;
		this.completedRecordMs.push(Date.now() - this.recordStartedAt);
		this.recordStartedAt = null;
	}

	destroy() {
		if (this.unlisten) this.unlisten();
	}
}

export const synthesisStore = new SynthesisStore();
