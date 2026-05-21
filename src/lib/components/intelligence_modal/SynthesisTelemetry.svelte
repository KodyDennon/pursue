<script lang="ts">
	import { Brain, Terminal, Activity, Loader2, Cpu, CheckCircle2 } from 'lucide-svelte';

	let {
		status,
		busy,
		currentRecordId,
		currentBatchIndex,
		totalBatchCount,
		modelDownloadProgress,
		modelDownloadMsg,
		neuralTelemetry,
		onDismiss
	} = $props<{
		status: string;
		busy: boolean;
		currentRecordId: string | null;
		currentBatchIndex: number;
		totalBatchCount: number;
		modelDownloadProgress: number;
		modelDownloadMsg: string;
		neuralTelemetry: any | null;
		onDismiss: () => void;
	}>();
</script>

<div class="dashboard-side">
	<div class="brain-visual-container">
		<div class="neural-network-glow" class:active={busy}>
			<Brain size={64} class="neural-brain {busy ? 'pulse-brain' : ''}" />
		</div>
		<span class="engine-state-label" class:busy={busy}>
			{status === 'loading-model'
				? 'WAKING MODEL'
				: status === 'synthesizing'
					? 'SYNTHESIZING'
					: status.toUpperCase()}
		</span>
	</div>

	<div class="details-cards">
		<!-- Target File Card -->
		<div class="info-card">
			<Terminal size={16} class="card-icon" />
			<div class="val">
				<span class="l">Target Record</span>
				<span class="v"
					>{currentRecordId ? currentRecordId.substring(0, 16) + '...' : 'None'}</span
				>
			</div>
		</div>

		<!-- Batch Progress Card -->
		{#if totalBatchCount > 1}
			<div class="info-card batch-card">
				<Activity size={16} class="card-icon batch-icon" />
				<div class="val">
					<span class="l">Batch Synthesis Progress</span>
					<span class="v">Record {currentBatchIndex} of {totalBatchCount}</span>
					<div class="batch-progress-bar-bg">
						<div
							class="batch-progress-bar-fill"
							style="width: {(currentBatchIndex / totalBatchCount) * 100}%"
						></div>
					</div>
				</div>
			</div>
		{/if}

		<!-- Telemetry Metrics Card -->
		{#if status === 'loading-model'}
			<div class="info-card loading-state">
				<Loader2 size={16} class="spin card-icon text-accent" />
				<div class="val">
					<span class="l">Initializing Engine</span>
					<span class="v select-all" style="font-size: 11px;"
						>{modelDownloadMsg || 'Allocating tensors...'}</span
					>
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
			onclick={onDismiss}
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

<style>
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
		0%,
		100% {
			transform: scale(1);
			opacity: 0.8;
		}
		50% {
			transform: scale(1.08);
			opacity: 1;
			filter: drop-shadow(0 0 20px rgba(231, 196, 107, 0.8));
		}
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
		border-bottom: 1px solid rgba(255, 255, 255, 0.03);
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

	.batch-card {
		border-color: rgba(80, 179, 255, 0.2);
		background: rgba(80, 179, 255, 0.02);
	}

	:global(.batch-icon) {
		color: #50b3ff;
	}

	.batch-progress-bar-bg {
		width: 100%;
		height: 4px;
		background: rgba(255, 255, 255, 0.05);
		border-radius: 2px;
		overflow: hidden;
		margin-top: 6px;
	}

	.batch-progress-bar-fill {
		height: 100%;
		background: #50b3ff;
		box-shadow: 0 0 8px #50b3ff;
		transition: width 0.3s ease-out;
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
