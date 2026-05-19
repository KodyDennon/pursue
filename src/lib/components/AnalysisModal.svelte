<script lang="ts">
	import { invoke } from '@tauri-apps/api/core';
	import { listen, type UnlistenFn } from '@tauri-apps/api/event';
	import { onMount } from 'svelte';
	import { Brain, X, Loader2, CheckCircle2, AlertCircle, Terminal, Activity } from 'lucide-svelte';
	import { logger } from '$lib/logger';

	let { isOpen = $bindable(false), onComplete } = $props<{
		isOpen: boolean;
		onComplete?: () => void;
	}>();

	interface AnalysisProgress {
		status: string;
		current?: number;
		total?: number;
		record_id?: string;
		error?: string;
		chunk_count?: number;
		token_text?: string;
		msg?: string;
		telemetry?: {
			input_shape: number[];
			kv_cache_shape: number[];
			device: string;
		};
	}

	let progress = $state(0);
	let status = $state('standby');
	let currentRecordId = $state<string | null>(null);
	let processedCount = $state(0);
	let totalCount = $state(0);
	let neuralTelemetry = $state<AnalysisProgress['telemetry'] | null>(null);
	interface LogEntry {
		id: string;
		time: string;
		msg: string;
		type: 'info' | 'error' | 'success';
	}

	let logs = $state<LogEntry[]>([]);
	let thoughtText = $state('');
	let busy = $state(false);

	function addLog(msg: string, type: 'info' | 'error' | 'success' = 'info') {
		const time = new Date().toLocaleTimeString([], {
			hour12: false,
			hour: '2-digit',
			minute: '2-digit',
			second: '2-digit'
		});
		logs = [{ id: crypto.randomUUID(), time, msg, type }, ...logs].slice(0, 50);
	}

	async function startAnalysis() {
		if (busy) return;
		busy = true;
		status = 'initializing';
		progress = 0;
		processedCount = 0;
		logs = [];
		addLog('Intelligence Foundation Engine starting...', 'info');
		addLog('Waking Neural Vision Sidecar (GOT-OCR-2.0)...', 'info');
		addLog('Binding to Apple Neural Engine (ANE)...', 'info');

		try {
			const cmd = 'analyze_all_records';
			const count = await invoke<number>(cmd);
			totalCount = count;
			if (count === 0) {
				addLog('No pending records found. Archive is already up-to-date.', 'success');
				status = 'completed';
				busy = false;
				return;
			}
			addLog(`Task queued: ${count} records identified for foundation indexing.`, 'info');
			status = 'processing';
		} catch (e) {
			addLog(`Initialization failed: ${e}`, 'error');
			status = 'failed';
			busy = false;
		}
	}

	onMount(() => {
		let unlisten: UnlistenFn;
		logger.debug('[AnalysisModal] Mounted, listening for progress...');

		listen<AnalysisProgress & { step?: string; engine?: string }>('analysis-progress', (event) => {
			const payload = event.payload;
			logger.debug('[AnalysisModal] Progress Event:', payload.status, payload);

			// Auto-activate and open modal if an event comes in from elsewhere
			const activeStatuses = [
				'starting',
				'processing',
				'analyzing',
				'thought',
				'extracting-foundation',
				'indexing-vector',
				'synthesizing'
			];
			if (activeStatuses.includes(payload.status)) {
				if (!isOpen) isOpen = true;
				busy = true;
				// Only update status if it's not already in a more specific state
				if (
					payload.status !== 'processing' ||
					(status !== 'extracting-foundation' &&
						status !== 'indexing-vector' &&
						status !== 'synthesizing')
				) {
					status =
						payload.status === 'analyzing'
							? 'processing'
							: payload.status === 'thought'
								? 'reasoning'
								: payload.status;
				}
			}

			processedCount = payload.current ?? processedCount;
			totalCount = payload.total ?? totalCount;
			currentRecordId = payload.record_id ?? currentRecordId;

			if (totalCount > 0) {
				progress = (processedCount / totalCount) * 100;
			}

			if (payload.status === 'batch-planning') {
				addLog(payload.msg || 'Batch planning...', 'info');
				thoughtText = (payload.msg || '') + '\n' + thoughtText;
				// Keep the thoughtText from growing indefinitely if it's just spamming download percentages
				if (thoughtText.length > 2000) {
					thoughtText = thoughtText.substring(0, 2000);
				}
			}

			if (payload.status === 'completed') {
				status = 'completed';
				busy = false;
				addLog('Neural Extraction Task Complete.', 'success');
				if (onComplete) onComplete();
			} else if (payload.status === 'thought') {
				addLog(
					`Initiating step-by-step reasoning for ${payload.record_id?.substring(0, 8)}...`,
					'info'
				);
			} else if (payload.status === 'failed') {
				status = 'failed';
				busy = false;
				addLog(`System Error: ${payload.error}`, 'error');
			} else if (payload.status === 'record-failed') {
				addLog(`Record ${payload.record_id?.substring(0, 8)} failed: ${payload.error}`, 'error');
			} else if (payload.status === 'starting' || payload.status === 'processing') {
				addLog(`Processing record: ${currentRecordId?.substring(0, 8)}...`, 'info');
			} else if (payload.status === 'extracting-foundation') {
				if (payload.step) {
					addLog(`OCR Trace: ${payload.step}`, 'info');
				} else if (status !== 'extracting-foundation') {
					addLog(`OCR Phase: Extracting foundation data...`, 'info');
					status = 'extracting-foundation';
				}
			} else if (payload.status === 'foundation-indexed') {
				addLog(`Foundation captured via ${payload.engine}.`, 'success');
			} else if (payload.status === 'indexing-vector') {
				if (status !== 'indexing-vector') {
					const chunkCount = payload.chunk_count ? ` (${payload.chunk_count} chunks)` : '';
					addLog(`Vector Phase: Mapping semantic associations${chunkCount}...`, 'info');
					status = 'indexing-vector';
				}
			} else if (payload.status === 'synthesizing' || payload.status === 'synthesizing-start') {
				if (status !== 'synthesizing') {
					if (payload.status === 'synthesizing-start') {
						addLog(`Intelligence Phase: Gemma 4 performing deep synthesis...`, 'info');
					}
					status = 'synthesizing';
					thoughtText = ''; // Reset for new record
				}
				if (payload.token_text) {
					thoughtText += payload.token_text;
				}
				if (payload.telemetry) {
					neuralTelemetry = payload.telemetry;
				}
			}
		}).then((u) => (unlisten = u));

		return () => {
			if (unlisten) unlisten();
		};
	});

	function close() {
		if (busy) {
			if (!confirm('Analysis is running in the background. Close window?')) return;
		}
		isOpen = false;
	}
</script>

{#if isOpen}
	<div class="modal-overlay">
		<div class="analysis-panel glass-panel">
			<header class="panel-header">
				<div class="brand">
					<Brain size={24} class="accent-icon" />
					<div>
						<h2>Intelligence Foundation Engine</h2>
						<p>High-resolution OCR and semantic vector indexing.</p>
					</div>
				</div>
				<button class="close-btn" onclick={close}><X size={20} /></button>
			</header>

			<div class="panel-body">
				<section class="status-overview">
					<div class="progress-wrap">
						<div class="stats-row">
							<span class="status-label">{status === 'standby' ? 'READY FOR INGESTION' : status.toUpperCase()}</span>
							<span class="count-label">{processedCount} / {totalCount} RECORDS</span>
						</div>
						<div class="progress-bar-bg">
							<div class="progress-bar-fill" style="width: {progress}%"></div>
							<div class="glow" style="left: {progress}%"></div>
						</div>
					</div>

					<div class="control-grid">
						<div class="info-card">
							<Activity size={18} />
							<div class="val">
								<span class="l">Current Unit</span>
								<span class="v"
									>{currentRecordId ? currentRecordId.substring(0, 12) + '...' : 'None'}</span
								>
							</div>
						</div>
						<div class="info-card" class:thinking={status === 'reasoning' || status === 'synthesizing'}>
							<Terminal size={18} />
							<div class="val">
								<span class="l">Status</span>
								<span class="v">
									{status === 'processing' || status === 'extracting-foundation'
										? 'OCR / FOUNDATION'
										: status === 'indexing-vector'
											? 'VECTORIZING'
											: status === 'synthesizing' || status === 'synthesizing-start' || status === 'analyzing'
												? 'NEURAL SYNTHESIS'
												: status === 'loading-model'
													? 'WAKING MODEL'
													: status.toUpperCase()}
								</span>
							</div>
						</div>
						<div class="action-wrap">
							<button
								class="start-btn"
								onclick={startAnalysis}
								disabled={busy || status === 'completed'}
							>
								{#if busy}
									<Loader2 size={18} class="spin" /> IN PROGRESS
								{:else if status === 'completed'}
									<CheckCircle2 size={18} /> TASK COMPLETE
								{:else}
									START BATCH INDEXING
								{/if}
							</button>
						</div>
					</div>
				</section>

				<div class="analysis-grid">
					<div class="log-section">
						<div class="section-head">
							<Terminal size={14} />
							<span>FOUNDATION OUTPUT LOG</span>
						</div>
						<div class="log-viewport custom-scrollbar">
							{#each logs as log (log.id)}
								<div class="log-entry {log.type}">
									<span class="log-time">[{log.time}]</span>
									<span class="log-msg">{log.msg}</span>
								</div>
							{/each}
						</div>
					</div>

					<div class="thought-section">
						<div class="section-head">
							<Activity size={14} />
							<span>NEURAL SYNTHESIS STREAM</span>
						</div>
						<div class="thought-viewport custom-scrollbar">
							{#if status === 'synthesizing' || status === 'reasoning'}
								<div class="neural-stream">
									<span class="cursor">█</span>
									{thoughtText}
								</div>
								{#if neuralTelemetry}
									<div class="neural-telemetry">
										<div class="t-row"><span>DEVICE</span> <strong>{neuralTelemetry.device}</strong></div>
										<div class="t-row"><span>INPUT</span> <strong>{JSON.stringify(neuralTelemetry.input_shape)}</strong></div>
										<div class="t-row"><span>KV_CACHE</span> <strong>{JSON.stringify(neuralTelemetry.kv_cache_shape)}</strong></div>
									</div>
								{/if}
							{:else}
								<div class="empty-state">Standby for intelligence synthesis (Dossier Mode only)...</div>
							{/if}
						</div>
					</div>
				</div>
			</div>

			<footer class="panel-footer">
				<div class="notice">
					<AlertCircle size={14} />
					<span
						>Indexing and OCR are hardware intensive. Do not close the application during
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
		max-width: 900px;
		height: 100%;
		max-height: 700px;
		background: #0a0b0d;
		border: 1px solid var(--border-subtle);
		display: flex;
		flex-direction: column;
		box-shadow: 0 30px 60px rgba(0, 0, 0, 0.8);
		overflow: hidden;
	}

	.panel-header {
		padding: 24px 32px;
		display: flex;
		justify-content: space-between;
		align-items: center;
		border-bottom: 1px solid var(--border-subtle);
	}

	.brand {
		display: flex;
		gap: 20px;
		align-items: center;
	}

	.brand h2 {
		margin: 0;
		font-size: 20px;
		letter-spacing: 0.05em;
	}
	.brand p {
		margin: 4px 0 0 0;
		font-size: 13px;
		color: var(--text-secondary);
	}

	.close-btn {
		background: none;
		border: none;
		color: var(--text-tertiary);
		cursor: pointer;
		padding: 8px;
		border-radius: 50%;
		transition: all 0.2s;
	}

	.close-btn:hover {
		background: rgba(255, 255, 255, 0.05);
		color: #fff;
	}

	.panel-body {
		flex: 1;
		padding: 32px;
		display: flex;
		flex-direction: column;
		gap: 32px;
		overflow: hidden;
	}

	.status-overview {
		display: flex;
		flex-direction: column;
		gap: 24px;
	}

	.progress-wrap {
		display: flex;
		flex-direction: column;
		gap: 12px;
	}

	.stats-row {
		display: flex;
		justify-content: space-between;
		font-size: 11px;
		font-weight: 800;
		letter-spacing: 0.1em;
	}

	.status-label {
		color: var(--accent-primary);
	}
	.count-label {
		color: var(--text-secondary);
	}

	.progress-bar-bg {
		height: 8px;
		background: rgba(255, 255, 255, 0.05);
		border-radius: 4px;
		position: relative;
		overflow: hidden;
	}

	.progress-bar-fill {
		height: 100%;
		background: var(--accent-primary);
		box-shadow: 0 0 15px var(--accent-primary);
		transition: width 0.4s cubic-bezier(0.4, 0, 0.2, 1);
	}

	.progress-bar-bg .glow {
		position: absolute;
		top: 0;
		width: 100px;
		height: 100%;
		background: linear-gradient(90deg, transparent, rgba(231, 196, 107, 0.4), transparent);
		transform: translateX(-50%);
		transition: left 0.4s ease;
	}

	.control-grid {
		display: grid;
		grid-template-columns: 1fr 1fr 240px;
		gap: 16px;
	}

	.info-card {
		background: rgba(255, 255, 255, 0.03);
		border: 1px solid var(--border-subtle);
		border-radius: var(--radius-md);
		padding: 16px;
		display: flex;
		align-items: center;
		gap: 16px;
		color: var(--text-secondary);
	}

	.info-card .val {
		display: flex;
		flex-direction: column;
		gap: 2px;
	}
	.info-card .l {
		font-size: 10px;
		text-transform: uppercase;
		font-weight: 700;
		opacity: 0.6;
	}
	.info-card .v {
		font-size: 14px;
		font-weight: 600;
		color: #fff;
		font-family: var(--font-mono);
	}

	.start-btn {
		background: var(--accent-primary);
		color: #000;
		border: none;
		border-radius: var(--radius-md);
		font-weight: 800;
		font-size: 13px;
		letter-spacing: 0.05em;
		cursor: pointer;
		display: flex;
		align-items: center;
		justify-content: center;
		gap: 10px;
		transition: all 0.2s;
	}

	.start-btn:hover:not(:disabled) {
		transform: scale(1.02);
		filter: brightness(1.1);
	}
	.start-btn:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	.action-wrap {
		display: flex;
		flex-direction: column;
		gap: 12px;
	}

	.info-card.thinking {

		border-color: #f3c46b;
		background: rgba(243, 196, 107, 0.05);
		animation: pulse-thought 2s infinite;
	}

	@keyframes pulse-thought {
		0% {
			box-shadow: 0 0 0 0 rgba(243, 196, 107, 0.2);
		}
		70% {
			box-shadow: 0 0 0 10px rgba(243, 196, 107, 0);
		}
		100% {
			box-shadow: 0 0 0 0 rgba(243, 196, 107, 0);
		}
	}

	.analysis-grid {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: 20px;
		flex: 1;
		min-height: 0;
	}

	.log-section,
	.thought-section {
		display: flex;
		flex-direction: column;
		gap: 12px;
		overflow: hidden;
	}

	.section-head {
		display: flex;
		align-items: center;
		gap: 8px;
		color: var(--text-tertiary);
		font-size: 10px;
		font-weight: 800;
		letter-spacing: 0.15em;
		text-transform: uppercase;
	}

	.log-viewport,
	.thought-viewport {
		flex: 1;
		background: rgba(0, 0, 0, 0.5);
		border: 1px solid var(--border-subtle);
		border-radius: var(--radius-md);
		padding: 16px;
		font-family: var(--font-mono);
		font-size: 11px;
		line-height: 1.6;
		overflow-y: auto;
	}

	.log-entry {
		display: flex;
		gap: 12px;
		margin-bottom: 4px;
	}

	.log-time {
		color: var(--text-tertiary);
		opacity: 0.5;
	}
	.log-entry.success {
		color: #4df3a9;
	}
	.log-entry.error {
		color: #f34d4d;
	}

	.neural-stream {
		color: var(--accent-primary);
		white-space: pre-wrap;
		word-break: break-all;
	}

	.neural-telemetry {
		margin-top: 20px;
		padding-top: 20px;
		border-top: 1px solid rgba(231, 196, 107, 0.1);
		display: flex;
		flex-direction: column;
		gap: 4px;
	}

	.t-row {
		display: flex;
		justify-content: space-between;
		font-size: 9px;
		color: var(--text-tertiary);
		font-family: var(--font-mono);
	}

	.t-row span {
		opacity: 0.5;
	}

	.t-row strong {
		color: var(--accent-primary);
	}

	.empty-state {
		height: 100%;
		display: flex;
		align-items: center;
		justify-content: center;
		color: var(--text-tertiary);
		opacity: 0.3;
		font-style: italic;
	}

	.cursor {
		display: inline-block;
		animation: blink 1s step-end infinite;
	}

	@keyframes blink {
		from,
		to {
			opacity: 1;
		}
		50% {
			opacity: 0;
		}
	}

	.panel-footer {
		padding: 20px 32px;
		background: rgba(0, 0, 0, 0.3);
		border-top: 1px solid var(--border-subtle);
	}

	.notice {
		display: flex;
		align-items: center;
		gap: 12px;
		color: var(--text-tertiary);
		font-size: 11px;
	}

	.accent-icon {
		color: var(--accent-primary);
	}
	:global(.spin) {
		animation: spin 1s linear infinite;
	}
	@keyframes spin {
		from {
			transform: rotate(0deg);
		}
		to {
			transform: rotate(360deg);
		}
	}
</style>
