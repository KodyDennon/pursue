<script lang="ts">
	import { invoke } from '@tauri-apps/api/core';
	import { listen, type UnlistenFn } from '@tauri-apps/api/event';
	import { onMount } from 'svelte';
	import { Layers, X, Loader2, CheckCircle2, AlertCircle, Terminal, Activity, FileText } from 'lucide-svelte';
	import { logger } from '$lib/logger';

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
					<!-- Dashboard Column (Left) -->
					<div class="dashboard-side">
						<section class="progress-wrap">
							<div class="stats-row">
								<span class="status-label">{status === 'standby' ? 'PIPELINE IDLE' : status.toUpperCase()}</span>
								<span class="count-label">{processedCount} / {totalCount} FILES</span>
							</div>
							<div class="progress-bar-bg">
								<div class="progress-bar-fill" style="width: {progress}%"></div>
								<div class="glow" style="left: {progress}%"></div>
							</div>
						</section>

						<div class="details-cards">
							<div class="info-card">
								<FileText size={18} class="card-icon" />
								<div class="val">
									<span class="l">Current Unit</span>
									<span class="v"
										>{currentRecordId ? currentRecordId.substring(0, 16) + '...' : 'None'}</span
									>
								</div>
							</div>

							<div class="info-card" class:indexing={busy}>
								<Activity size={18} class="card-icon pulse-active" />
								<div class="val">
									<span class="l">Status Engine</span>
									<span class="v">
										{#if status === 'processing'}
											OCR EXTRACTION
										{:else if status === 'vectorizing'}
											VECTOR ENCODING
										{:else if status === 'completed'}
											INDEX COMPLETED
										{:else if status === 'initializing'}
											WAKING ENGINES
										{:else}
											READY
										{/if}
									</span>
								</div>
							</div>
						</div>

						<div class="action-wrap">
							<button
								class="start-btn"
								onclick={startAnalysis}
								disabled={busy || status === 'completed'}
							>
								{#if busy}
									<Loader2 size={18} class="spin" /> INDEXING ACTIVE
								{:else if status === 'completed'}
									<CheckCircle2 size={18} /> PROCESS COMPLETE
								{:else}
									START BATCH INDEXING
								{/if}
							</button>
						</div>
					</div>

					<!-- Terminals Logs Column (Right) -->
					<div class="log-side">
						<div class="section-head">
							<Terminal size={14} />
							<span>Foundation Output Log</span>
						</div>
						<div class="log-viewport custom-scrollbar">
							{#if logs.length === 0}
								<div class="empty-state">Secure ingestion logs will stream here.</div>
							{:else}
								{#each logs as log (log.id)}
									<div class="log-entry {log.type}">
										<span class="log-time">[{log.time}]</span>
										<span class="log-msg">{log.msg}</span>
									</div>
								{/each}
							{/if}
						</div>
					</div>
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

	.dashboard-side {
		display: flex;
		flex-direction: column;
		gap: 24px;
		justify-content: space-between;
	}

	.progress-wrap {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}

	.stats-row {
		display: flex;
		justify-content: space-between;
		font-size: 10px;
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
		height: 6px;
		background: rgba(255, 255, 255, 0.04);
		border-radius: 3px;
		position: relative;
		overflow: hidden;
	}

	.progress-bar-fill {
		height: 100%;
		background: var(--accent-primary);
		box-shadow: 0 0 12px var(--accent-primary);
		transition: width 0.3s ease-out;
	}

	.progress-bar-bg .glow {
		position: absolute;
		top: 0;
		width: 60px;
		height: 100%;
		background: linear-gradient(90deg, transparent, rgba(231, 196, 107, 0.3), transparent);
		transform: translateX(-50%);
		transition: left 0.3s ease;
	}

	.details-cards {
		display: flex;
		flex-direction: column;
		gap: 12px;
		flex: 1;
		justify-content: center;
	}

	.info-card {
		background: rgba(255, 255, 255, 0.02);
		border: 1px solid var(--border-subtle);
		border-radius: var(--radius-md);
		padding: 14px 16px;
		display: flex;
		align-items: center;
		gap: 14px;
		color: var(--text-secondary);
		transition: border-color 0.2s;
	}

	.info-card.indexing {
		border-color: rgba(231, 196, 107, 0.2);
		background: rgba(231, 196, 107, 0.02);
	}

	.card-icon {
		color: var(--text-tertiary);
	}

	.info-card.indexing :global(.pulse-active) {
		color: var(--accent-primary);
		animation: pulse-light 1.5s infinite ease-in-out;
	}

	@keyframes pulse-light {
		0%, 100% { opacity: 0.6; }
		50% { opacity: 1; transform: scale(1.05); }
	}

	.info-card .val {
		display: flex;
		flex-direction: column;
		gap: 2px;
		min-width: 0;
	}
	.info-card .l {
		font-size: 9px;
		text-transform: uppercase;
		font-weight: 700;
		opacity: 0.5;
		letter-spacing: 0.05em;
	}
	.info-card .v {
		font-size: 13px;
		font-weight: 600;
		color: #fff;
		font-family: var(--font-mono);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.action-wrap {
		display: flex;
		flex-direction: column;
	}

	.start-btn {
		width: 100%;
		height: 44px;
		background: var(--accent-primary);
		color: #000;
		border: none;
		border-radius: var(--radius-md);
		font-weight: 700;
		font-size: 13px;
		letter-spacing: 0.05em;
		cursor: pointer;
		display: flex;
		align-items: center;
		justify-content: center;
		gap: 8px;
		transition: all 0.2s;
		box-shadow: 0 4px 15px rgba(231, 196, 107, 0.2);
	}

	.start-btn:hover:not(:disabled) {
		transform: translateY(-1px);
		filter: brightness(1.1);
		box-shadow: 0 6px 20px rgba(231, 196, 107, 0.35);
	}
	.start-btn:disabled {
		opacity: 0.5;
		cursor: not-allowed;
		box-shadow: none;
	}

	.log-side {
		display: flex;
		flex-direction: column;
		gap: 10px;
		overflow: hidden;
		border-left: 1px solid var(--border-subtle);
		padding-left: 28px;
	}

	.section-head {
		display: flex;
		align-items: center;
		gap: 8px;
		color: var(--text-tertiary);
		font-size: 9px;
		font-weight: 800;
		letter-spacing: 0.15em;
		text-transform: uppercase;
	}

	.log-viewport {
		flex: 1;
		background: rgba(0, 0, 0, 0.4);
		border: 1px solid var(--border-subtle);
		border-radius: var(--radius-md);
		padding: 16px;
		font-family: var(--font-mono);
		font-size: 11px;
		line-height: 1.5;
		overflow-y: auto;
		display: flex;
		flex-direction: column-reverse;
	}

	.empty-state {
		height: 100%;
		display: flex;
		align-items: center;
		justify-content: center;
		color: var(--text-tertiary);
		opacity: 0.5;
		font-style: italic;
	}

	.log-entry {
		display: flex;
		gap: 12px;
		margin-bottom: 5px;
		word-break: break-all;
	}

	.log-time {
		color: var(--text-tertiary);
		opacity: 0.6;
		flex-shrink: 0;
	}
	.log-msg {
		color: var(--text-secondary);
	}
	.log-entry.success .log-msg {
		color: var(--accent-success);
	}
	.log-entry.error .log-msg {
		color: var(--accent-danger);
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

	.accent-icon {
		color: var(--accent-primary);
	}
	:global(.spin) {
		animation: spin 1s linear infinite;
	}
	@keyframes spin {
		from { transform: rotate(0deg); }
		to { transform: rotate(360deg); }
	}
</style>
