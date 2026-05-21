<script lang="ts">
	import { listen, type UnlistenFn } from '@tauri-apps/api/event';
	import { onMount } from 'svelte';
	import { Brain, X, Loader2, CheckCircle2, AlertCircle, Terminal, Activity, Cpu } from 'lucide-svelte';
	import { logger } from '$lib/logger';

	let {
		isOpen = $bindable(false),
		isBusy = $bindable(false),
		onComplete
	} = $props<{
		isOpen: boolean;
		isBusy?: boolean;
		onComplete?: () => void;
	}>();

	interface AnalysisProgress {
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

	let status = $state('standby');
	let currentRecordId = $state<string | null>(null);
	let neuralTelemetry = $state<AnalysisProgress['telemetry'] | null>(null);
	let thoughtText = $state('');
	let busy = $state(false);

	$effect(() => {
		isBusy = busy;
	});
	
	let modelDownloadProgress = $state(0);
	let modelDownloadMsg = $state('');

	onMount(() => {
		let unlisten: UnlistenFn;
		logger.debug('[IntelligenceModal] Listening for neural synthesis events...');

		listen<AnalysisProgress>('analysis-progress', (event) => {
			const payload = event.payload;

			// Handle intelligence-specific active statuses
			const intelligenceStatuses = [
				'loading-model',
				'synthesizing-start',
				'synthesizing'
			];

			if (intelligenceStatuses.includes(payload.status)) {
				if (!isOpen) {
					isOpen = true;
				}
				busy = true;
				status = payload.status === 'synthesizing-start'
					? 'synthesizing'
					: payload.status;
			} else if (payload.status === 'completed') {
				if (busy) {
					status = 'completed';
					busy = false;
					if (onComplete) onComplete();
				}
				return;
			} else if (payload.status === 'failed') {
				if (busy) {
					status = 'failed';
					busy = false;
				}
				return;
			} else {
				// Ignore OCR/Foundation indexing events completely in this modal
				return;
			}

			currentRecordId = payload.record_id ?? currentRecordId;

			if (payload.status === 'loading-model') {
				modelDownloadProgress = payload.progress ?? modelDownloadProgress;
				modelDownloadMsg = payload.msg ?? modelDownloadMsg;
			} else if (payload.status === 'synthesizing-start') {
				thoughtText = ''; // Reset for new synthesis
				neuralTelemetry = null;
			} else if (payload.status === 'synthesizing') {
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
		isOpen = false;
	}
</script>

{#if isOpen}
	<div class="modal-overlay">
		<div class="synthesis-panel glass-panel">
			<header class="panel-header glass-header">
				<div class="brand">
					<Brain size={24} class="accent-icon pulse-active" />
					<div>
						<h2>Cognitive Synthesis Terminal</h2>
						<p>Gemma 4 Deep Intelligence Generation & Forensic Audit</p>
					</div>
				</div>
				<button class="close-btn" onclick={close} aria-label="Close terminal"><X size={20} /></button>
			</header>

			<div class="panel-body">
				<div class="overhaul-grid">
					<!-- Telemetry Dashboard Column (Left) -->
					<div class="dashboard-side">
						<div class="brain-visual-container">
							<div class="neural-network-glow" class:active={busy}>
								<Brain size={64} class="neural-brain {busy ? 'pulse-brain' : ''}" />
							</div>
							<span class="engine-state-label" class:busy={busy}>
								{status === 'loading-model' ? 'WAKING MODEL' : status === 'synthesizing' ? 'SYNTHESIZING' : status.toUpperCase()}
							</span>
						</div>

						<div class="details-cards">
							<!-- Target File Card -->
							<div class="info-card">
								<Terminal size={16} class="card-icon" />
								<div class="val">
									<span class="l">Target Record</span>
									<span class="v">{currentRecordId ? currentRecordId.substring(0, 16) + '...' : 'None'}</span>
								</div>
							</div>

							<!-- Telemetry Metrics Card -->
							{#if status === 'loading-model'}
								<div class="info-card loading-state">
									<Loader2 size={16} class="spin card-icon text-accent" />
									<div class="val">
										<span class="l">Initializing Engine</span>
										<span class="v select-all" style="font-size: 11px;">{modelDownloadMsg || 'Allocating tensors...'}</span>
									</div>
								</div>
								{#if modelDownloadProgress > 0}
									<div class="model-progress-wrap">
										<div class="model-progress-bg">
											<div class="model-progress-fill" style="width: {modelDownloadProgress}%"></div>
										</div>
										<span class="model-progress-text">{modelDownloadProgress.toFixed(1)}%</span>
									</div>
								{/if}
							{:else if neuralTelemetry}
								<div class="info-card telemetry-card">
									<Cpu size={16} class="card-icon telemetry-icon" />
									<div class="val text-row-stack">
										<div class="telemetry-row">
											<span>DEVICE</span>
											<strong>{neuralTelemetry.device.replace('Device::', '')}</strong>
										</div>
										<div class="telemetry-row">
											<span>INPUT SHAPE</span>
											<strong>{JSON.stringify(neuralTelemetry.input_shape)}</strong>
										</div>
										<div class="telemetry-row">
											<span>KV CACHE SHAPE</span>
											<strong>{JSON.stringify(neuralTelemetry.kv_cache_shape)}</strong>
										</div>
									</div>
								</div>
							{:else}
								<div class="info-card standby-card">
									<Activity size={16} class="card-icon" />
									<div class="val">
										<span class="l">Neural Telemetry</span>
										<span class="v">Awaiting Inference</span>
									</div>
								</div>
							{/if}
						</div>

						<div class="action-wrap">
							<button
								class="dismiss-btn"
								class:completed={status === 'completed'}
								onclick={() => isOpen = false}
								disabled={busy}
							>
								{#if busy}
									<Loader2 size={16} class="spin" /> SYNTHESIS RUNNING
								{:else if status === 'completed'}
									<CheckCircle2 size={16} /> DISMISS TERMINAL
								{:else}
									DISMISS
								{/if}
							</button>
						</div>
					</div>

					<!-- live stream Column (Right) -->
					<div class="stream-side">
						<div class="section-head">
							<Activity size={14} />
							<span>Cognitive Thought Block</span>
						</div>
						<div class="stream-viewport custom-scrollbar">
							{#if status === 'loading-model'}
								<div class="model-loading-fullscreen">
									<Brain size={48} class="accent-icon pulse-brain" />
									<h3>PROVISIONING NEURAL RUNTIME</h3>
									<p>{modelDownloadMsg || 'Mounting tensor files into hardware cache...'}</p>
								</div>
							{:else if thoughtText}
								<div class="neural-stream">
									{thoughtText}<span class="cursor">█</span>
								</div>
							{:else}
								<div class="empty-state">Thought stream will manifest here upon inference start.</div>
							{/if}
						</div>
					</div>
				</div>
			</div>

			<footer class="panel-footer">
				<div class="notice">
					<AlertCircle size={14} />
					<span>Neural inference utilizes Apple Neural Engine or local GPU. Keep application active.</span>
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

	.synthesis-panel {
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

	.brain-visual-container {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 14px;
		padding: 16px 0;
		background: rgba(255, 255, 255, 0.01);
		border-radius: var(--radius-md);
		border: 1px solid var(--border-subtle);
	}

	.neural-network-glow {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 100px;
		height: 100px;
		border-radius: 50%;
		background: rgba(255, 255, 255, 0.02);
		border: 1px solid rgba(255, 255, 255, 0.04);
		transition: all 0.5s ease;
	}

	.neural-network-glow.active {
		border-color: rgba(231, 196, 107, 0.3);
		box-shadow: 0 0 30px rgba(231, 196, 107, 0.15), inset 0 0 20px rgba(231, 196, 107, 0.05);
		background: rgba(231, 196, 107, 0.02);
	}

	:global(.neural-brain) {
		color: var(--text-tertiary);
		transition: color 0.5s;
	}

	.neural-network-glow.active :global(.neural-brain) {
		color: var(--accent-primary);
	}

	:global(.pulse-brain) {
		animation: pulse-brain-anim 2.5s infinite ease-in-out;
		filter: drop-shadow(0 0 12px rgba(231, 196, 107, 0.5));
	}

	@keyframes pulse-brain-anim {
		0%, 100% { transform: scale(1); opacity: 0.8; }
		50% { transform: scale(1.08); opacity: 1; filter: drop-shadow(0 0 20px rgba(231, 196, 107, 0.8)); }
	}

	.engine-state-label {
		font-size: 10px;
		font-weight: 800;
		letter-spacing: 0.15em;
		color: var(--text-tertiary);
	}

	.engine-state-label.busy {
		color: var(--accent-primary);
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
	}

	:global(.card-icon) {
		color: var(--text-tertiary);
		flex-shrink: 0;
	}

	.info-card .val {
		display: flex;
		flex-direction: column;
		gap: 2px;
		min-width: 0;
		width: 100%;
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

	.telemetry-card {
		align-items: flex-start;
	}
	:global(.telemetry-icon) {
		margin-top: 2px;
	}

	.text-row-stack {
		display: flex;
		flex-direction: column;
		gap: 8px !important;
	}

	.telemetry-row {
		display: flex;
		justify-content: space-between;
		width: 100%;
		font-family: var(--font-mono);
		font-size: 9px;
		border-bottom: 1px solid rgba(255,255,255,0.03);
		padding-bottom: 4px;
	}
	.telemetry-row:last-child {
		border-bottom: none;
		padding-bottom: 0;
	}

	.telemetry-row span {
		color: var(--text-tertiary);
		font-weight: 500;
	}

	.telemetry-row strong {
		color: var(--accent-primary);
		font-weight: 600;
	}

	.model-progress-wrap {
		width: 100%;
		display: flex;
		align-items: center;
		gap: 12px;
		padding: 0 4px;
	}

	.model-progress-bg {
		flex: 1;
		height: 4px;
		background: rgba(255, 255, 255, 0.05);
		border-radius: 2px;
		overflow: hidden;
	}

	.model-progress-fill {
		height: 100%;
		background: var(--accent-primary);
		box-shadow: 0 0 10px var(--accent-primary);
		transition: width 0.3s ease-out;
	}

	.model-progress-text {
		font-family: var(--font-mono);
		color: var(--accent-primary);
		font-size: 10px;
		font-weight: 700;
		width: 40px;
		text-align: right;
	}

	.action-wrap {
		display: flex;
		flex-direction: column;
	}

	.dismiss-btn {
		width: 100%;
		height: 44px;
		background: rgba(255, 255, 255, 0.05);
		border: 1px solid var(--border-subtle);
		color: var(--text-secondary);
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
	}

	.dismiss-btn.completed {
		background: var(--accent-primary);
		color: #000;
		box-shadow: 0 4px 15px rgba(231, 196, 107, 0.2);
		border: none;
	}

	.dismiss-btn.completed:hover {
		transform: translateY(-1px);
		filter: brightness(1.1);
		box-shadow: 0 6px 20px rgba(231, 196, 107, 0.35);
	}

	.dismiss-btn:disabled {
		opacity: 0.5;
		cursor: not-allowed;
		box-shadow: none;
	}

	.stream-side {
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

	.stream-viewport {
		flex: 1;
		background: rgba(0, 0, 0, 0.4);
		border: 1px solid var(--border-subtle);
		border-radius: var(--radius-md);
		padding: 20px;
		overflow-y: auto;
	}

	.model-loading-fullscreen {
		height: 100%;
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		text-align: center;
		gap: 16px;
	}

	.model-loading-fullscreen h3 {
		font-size: 13px;
		letter-spacing: 0.1em;
		color: var(--text-primary);
	}

	.model-loading-fullscreen p {
		font-size: 11px;
		font-family: var(--font-mono);
		color: var(--text-secondary);
		opacity: 0.8;
	}

	.neural-stream {
		font-family: var(--font-mono);
		font-size: 12px;
		line-height: 1.7;
		color: var(--accent-primary);
		white-space: pre-wrap;
		word-break: break-all;
		text-shadow: 0 0 5px rgba(231, 196, 107, 0.2);
	}

	.empty-state {
		height: 100%;
		display: flex;
		align-items: center;
		justify-content: center;
		color: var(--text-tertiary);
		opacity: 0.5;
		font-style: italic;
		font-size: 11px;
	}

	.cursor {
		display: inline-block;
		animation: blink 1s step-end infinite;
		color: var(--accent-primary);
	}

	@keyframes blink {
		from, to { opacity: 1; }
		50% { opacity: 0; }
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
