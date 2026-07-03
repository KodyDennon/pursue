<script lang="ts">
	import { FileText, Activity, Loader2, CheckCircle2 } from 'lucide-svelte';
	import { analysisStore } from '$lib/stores/analysisStore.svelte';

	let {
		status,
		processedCount,
		totalCount,
		progress,
		currentRecordId,
		busy,
		ocrDownloadProgress = 0,
		ocrDownloadMsg = '',
		onStartAnalysis
	} = $props<{
		status: string;
		processedCount: number;
		totalCount: number;
		progress: number;
		currentRecordId: string | null;
		busy: boolean;
		ocrDownloadProgress?: number;
		ocrDownloadMsg?: string;
		onStartAnalysis: () => void;
	}>();
</script>

<div class="dashboard-side">
	<section class="progress-wrap">
		<div class="stats-row">
			<span class="status-label">
				{status === 'standby'
					? 'PIPELINE IDLE'
					: status === 'loading-ocr-engine'
						? 'NEURAL SETUP'
						: status.toUpperCase().replace('-', ' ')}
			</span>
			<span class="count-label">{processedCount} / {totalCount} FILES</span>
		</div>
		<div class="progress-bar-bg">
			<div class="progress-bar-fill" style="width: {progress}%"></div>
			<div class="glow" style="left: {progress}%"></div>
		</div>

		{#if status === 'loading-ocr-engine' || ocrDownloadProgress > 0}
			<div class="ocr-download-wrap">
				<div class="ocr-stats-row">
					<span class="ocr-status-label">NEURAL VISION ENGINE SETUP</span>
					<span class="ocr-count-label">{ocrDownloadProgress.toFixed(1)}%</span>
				</div>
				<div class="progress-bar-bg sub-bar">
					<div class="progress-bar-fill ocr-fill" style="width: {ocrDownloadProgress}%"></div>
				</div>
				{#if ocrDownloadMsg}
					<div class="ocr-msg" title={ocrDownloadMsg}>{ocrDownloadMsg}</div>
				{/if}
			</div>
		{/if}
	</section>

	<div class="details-cards">
		<div class="info-card">
			<FileText size={18} class="card-icon" />
			<div class="val">
				<span class="l">Current Unit</span>
				<span class="v">{currentRecordId ? currentRecordId.substring(0, 16) + '...' : 'None'}</span>
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
					{:else if status === 'loading-ocr-engine'}
						NEURAL VISION SETUP
					{:else}
						READY
					{/if}
				</span>
			</div>
		</div>
	</div>

	<div class="action-wrap">
		{#if busy}
			<button class="abort-btn" onclick={() => analysisStore.abortAnalysis()}>
				<Loader2 size={18} class="spin" /> ABORT BATCH
			</button>
		{:else}
			<button class="start-btn" onclick={onStartAnalysis} disabled={busy || status === 'completed'}>
				{#if status === 'completed'}
					<CheckCircle2 size={18} /> PROCESS COMPLETE
				{:else}
					START BATCH INDEXING
				{/if}
			</button>
		{/if}
	</div>
</div>

<style>
	.dashboard-side {
		display: flex;
		flex-direction: column;
		gap: var(--space-5xl);
		justify-content: space-between;
		height: 100%;
	}

	.progress-wrap {
		display: flex;
		flex-direction: column;
		gap: var(--space-md);
	}

	.stats-row {
		display: flex;
		justify-content: space-between;
		font-size: var(--text-xs);
		font-weight: 800;
		letter-spacing: 0.1em;
	}

	.status-label {
		color: var(--color-accent-primary);
	}
	.count-label {
		color: var(--color-text-secondary);
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
		background: var(--color-accent-primary);
		box-shadow: 0 0 12px var(--color-accent-primary);
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
		gap: var(--space-xl);
		flex: 1;
		justify-content: center;
	}

	.info-card {
		background: rgba(255, 255, 255, 0.02);
		border: 1px solid var(--color-border-subtle);
		border-radius: var(--radius-md);
		padding: 14px 16px;
		display: flex;
		align-items: center;
		gap: var(--space-2xl);
		color: var(--color-text-secondary);
		transition: border-color 0.2s;
	}

	.info-card.indexing {
		border-color: rgba(231, 196, 107, 0.2);
		background: rgba(231, 196, 107, 0.02);
	}

	:global(.card-icon) {
		color: var(--color-text-tertiary);
	}

	.info-card.indexing :global(.pulse-active) {
		color: var(--color-accent-primary);
		animation: pulse-light 1.5s infinite ease-in-out;
	}

	@keyframes pulse-light {
		0%,
		100% {
			opacity: 0.6;
		}
		50% {
			opacity: 1;
			transform: scale(1.05);
		}
	}

	.info-card .val {
		display: flex;
		flex-direction: column;
		gap: var(--space-2xs);
		min-width: 0;
	}
	.info-card .l {
		font-size: var(--text-2xs);
		text-transform: uppercase;
		font-weight: 700;
		opacity: 0.5;
		letter-spacing: 0.05em;
	}
	.info-card .v {
		font-size: var(--text-md);
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
		background: var(--color-accent-primary);
		color: #000;
		border: none;
		border-radius: var(--radius-md);
		font-weight: 700;
		font-size: var(--text-md);
		letter-spacing: 0.05em;
		cursor: pointer;
		display: flex;
		align-items: center;
		justify-content: center;
		gap: var(--space-md);
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

	.abort-btn {
		width: 100%;
		height: 44px;
		background: rgba(255, 68, 68, 0.1);
		color: var(--color-accent-danger);
		border: 1px solid rgba(255, 68, 68, 0.2);
		border-radius: var(--radius-md);
		font-weight: 700;
		font-size: var(--text-md);
		letter-spacing: 0.05em;
		cursor: pointer;
		display: flex;
		align-items: center;
		justify-content: center;
		gap: var(--space-md);
		transition: all 0.2s;
	}

	.abort-btn:hover {
		background: rgba(255, 68, 68, 0.2);
		border-color: var(--color-accent-danger);
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

	.ocr-download-wrap {
		display: flex;
		flex-direction: column;
		gap: var(--space-sm);
		margin-top: var(--space-md);
		padding: var(--space-lg);
		background: rgba(255, 255, 255, 0.02);
		border: 1px dashed rgba(231, 196, 107, 0.2);
		border-radius: var(--radius-sm);
	}

	.ocr-stats-row {
		display: flex;
		justify-content: space-between;
		font-size: var(--text-2xs);
		font-weight: 800;
		letter-spacing: 0.05em;
	}

	.ocr-status-label {
		color: var(--color-accent-primary);
	}

	.ocr-count-label {
		color: var(--color-text-secondary);
	}

	.ocr-msg {
		font-size: var(--text-xs);
		color: var(--color-text-tertiary);
		font-family: var(--font-mono);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.sub-bar {
		height: 4px;
	}

	.ocr-fill {
		background: var(--color-accent-primary);
	}
</style>
