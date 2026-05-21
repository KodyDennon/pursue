<script lang="ts">
	import { invoke } from '@tauri-apps/api/core';
	import { listen, type UnlistenFn } from '@tauri-apps/api/event';
	import { onMount } from 'svelte';
	import { Layers, X, AlertCircle } from 'lucide-svelte';
	import { logger } from '$lib/logger';

	import NeuralTelemetry from './analysis_modal/NeuralTelemetry.svelte';
	import ThoughtStream from './analysis_modal/ThoughtStream.svelte';

	let {
		isOpen = $bindable(false),
		isBusy = $bindable(false),
		progress = $bindable(0),
		onComplete
	} = $props<{
		isOpen: boolean;
		isBusy?: boolean;
		progress?: number;
		onComplete?: () => void;
	}>();

	interface AnalysisProgress {
		status: string;
		current?: number;
		total?: number;
		record_id?: string;
		error?: string;
		chunk_count?: number;
		msg?: string;
		progress?: number;
		engine?: string;
		step?: string;
	}

	interface LogEntry {
		id: string;
		time: string;
		msg: string;
		type: 'info' | 'error' | 'success';
	}

	let status = $state('standby');
	let currentRecordId = $state<string | null>(null);
	let processedCount = $state(0);
	let totalCount = $state(0);
	let logs = $state<LogEntry[]>([]);
	let busy = $state(false);

	$effect(() => {
		isBusy = busy;
	});

	function addLog(msg: string, type: 'info' | 'error' | 'success' = 'info') {
		const time = new Date().toLocaleTimeString([], {
			hour12: false,
			hour: '2-digit',
			minute: '2-digit',
			second: '2-digit'
		});
		logs = [{ id: crypto.randomUUID(), time, msg, type }, ...logs].slice(0, 100);
	}

	async function startAnalysis() {
		if (busy) return;
		busy = true;
		status = 'initializing';
		progress = 0;
		processedCount = 0;
		logs = [];
		addLog('Secure Ingestion Pipeline initialized...', 'info');
		addLog('Starting Optical Character Recognition (OCR) runtime...', 'info');
		addLog('Allocating vector engine indices...', 'info');

		try {
			const cmd = 'analyze_all_records';
			const count = await invoke<number>(cmd);
			totalCount = count;
			if (count === 0) {
				addLog('Zero pending records identified. Database is fully audited.', 'success');
				status = 'completed';
				busy = false;
				return;
			}
			addLog(`Batch Queued: ${count} target files registered for ingestion.`, 'info');
			status = 'processing';
		} catch (e) {
			addLog(`Ingestion failed to start: ${e}`, 'error');
			status = 'failed';
			busy = false;
		}
	}

	onMount(() => {
		let unlisten: UnlistenFn;
		logger.debug('[AnalysisModal] Listening for foundation indexing events...');

		listen<AnalysisProgress>('analysis-progress', (event) => {
			const payload = event.payload;

			// Handle foundation-specific active statuses
			const ocrStatuses = [
				'initializing-batch',
				'batch-planning',
				'extracting-foundation',
				'foundation-indexed',
				'indexing-vector',
				'record-completed',
				'record-failed'
			];

			if (ocrStatuses.includes(payload.status)) {
				if (
					payload.status === 'initializing-batch' ||
					(payload.status === 'extracting-foundation' && payload.current === 1)
				) {
					logs = [];
					progress = 0;
					processedCount = 0;
					totalCount = 0;
					currentRecordId = null;
				}
				if (!isOpen) {
					isOpen = true;
				}
				busy = true;
				status = payload.status === 'extracting-foundation'
					? 'processing'
					: payload.status === 'indexing-vector'
						? 'vectorizing'
						: payload.status;
			} else if (payload.status === 'completed') {
				// Only complete if we were actively running this modal's pipeline
				if (busy) {
					status = 'completed';
					busy = false;
					addLog('Ingestion and Vector Indexing complete.', 'success');
					if (onComplete) onComplete();
				}
				return;
			} else if (payload.status === 'failed') {
				if (busy) {
					status = 'failed';
					busy = false;
					addLog(`Process aborted: ${payload.error}`, 'error');
				}
				return;
			} else {
				// Ignore intelligence synthesis events completely in this modal
				return;
			}

			processedCount = payload.current ?? processedCount;
			totalCount = payload.total ?? totalCount;
			currentRecordId = payload.record_id ?? currentRecordId;

			if (totalCount > 0) {
				progress = (processedCount / totalCount) * 100;
			}

			if (payload.status === 'batch-planning') {
				addLog(payload.msg || 'Organizing batch execution plan...', 'info');
			} else if (payload.status === 'initializing-batch') {
				addLog(payload.msg || 'Resolving ingestion pipeline settings...', 'info');
			} else if (payload.status === 'extracting-foundation') {
				if (payload.step) {
					addLog(`[OCR Trace] ${payload.step}`, 'info');
				} else {
					addLog(`Analyzing document structures for record: ${payload.record_id?.substring(0, 12)}...`, 'info');
				}
			} else if (payload.status === 'foundation-indexed') {
				addLog(`Foundation metadata mapped via ${payload.engine || 'native OCR'}.`, 'success');
			} else if (payload.status === 'indexing-vector') {
				const chunkMsg = payload.chunk_count ? ` (${payload.chunk_count} semantic associations)` : '';
				addLog(`Chunking and embedding text vectors${chunkMsg}...`, 'info');
			} else if (payload.status === 'record-completed') {
				addLog(`Record ${payload.record_id?.substring(0, 8)} successfully secured in the vault.`, 'success');
			} else if (payload.status === 'record-failed') {
				addLog(`Failed to index record ${payload.record_id?.substring(0, 8)}: ${payload.error}`, 'error');
			}
		}).then((u) => (unlisten = u));

		return () => {
			if (unlisten) unlisten();
		};
	});

	function close() {
		isOpen = false;
	}
</script>

{#if isOpen}
	<div class="modal-overlay">
		<div class="analysis-panel glass-panel">
			<header class="panel-header glass-header">
				<div class="brand">
					<Layers size={24} class="accent-icon" />
					<div>
						<h2>Secure Ingestion & Foundation Audit</h2>
						<p>High-resolution OCR extraction and semantic vector mapping.</p>
					</div>
				</div>
				<button class="close-btn" onclick={close} aria-label="Close modal"><X size={20} /></button>
			</header>

			<div class="panel-body">
				<div class="overhaul-grid">
					<NeuralTelemetry
						{status}
						{processedCount}
						{totalCount}
						{progress}
						{currentRecordId}
						{busy}
						onStartAnalysis={startAnalysis}
					/>

					<ThoughtStream {logs} />
				</div>
			</div>

			<footer class="panel-footer">
				<div class="notice">
					<AlertCircle size={14} />
					<span
						>Ingestion and OCR are hardware intensive. Do not close the application during
						active processing.</span
					>
				</div>
			</footer>
		</div>
	</div>
{/if}

<style>
	.modal-overlay {
		position: fixed;
		inset: 0;
		z-index: 2000;
		background: rgba(0, 0, 0, 0.85);
		backdrop-filter: blur(10px);
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 40px;
	}

	.analysis-panel {
		width: 100%;
		max-width: 960px;
		height: 100%;
		max-height: 620px;
		display: flex;
		flex-direction: column;
		overflow: hidden;
	}

	.panel-header {
		padding: 20px 28px;
		display: flex;
		justify-content: space-between;
		align-items: center;
		border-bottom: 1px solid var(--border-subtle);
	}

	.brand {
		display: flex;
		gap: 16px;
		align-items: center;
	}

	.brand h2 {
		margin: 0;
		font-size: 18px;
		font-weight: 600;
		letter-spacing: 0.02em;
	}
	.brand p {
		margin: 2px 0 0 0;
		font-size: 12px;
		color: var(--text-secondary);
	}

	.close-btn {
		background: none;
		border: none;
		color: var(--text-tertiary);
		cursor: pointer;
		padding: 6px;
		border-radius: 50%;
		transition: all 0.2s;
	}

	.close-btn:hover {
		background: rgba(255, 255, 255, 0.05);
		color: #fff;
	}

	.panel-body {
		flex: 1;
		padding: 28px;
		overflow: hidden;
	}

	.overhaul-grid {
		display: grid;
		grid-template-columns: 320px 1fr;
		gap: 28px;
		height: 100%;
	}

	.panel-footer {
		padding: 16px 28px;
		background: rgba(0, 0, 0, 0.2);
		border-top: 1px solid var(--border-subtle);
	}

	.notice {
		display: flex;
		align-items: center;
		gap: 10px;
		color: var(--text-tertiary);
		font-size: 11px;
	}

	:global(.accent-icon) {
		color: var(--accent-primary);
	}
</style>
