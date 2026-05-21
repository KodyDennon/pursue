<script lang="ts">
	let {
		statusText,
		progressWidth,
		currentModelName,
		overallProgress,
		speedMbps,
		etaSeconds,
		currentModelIndex,
		totalModels
	} = $props<{
		statusText: string;
		progressWidth: number;
		currentModelName: string;
		overallProgress: number;
		speedMbps: number | null;
		etaSeconds: number | null;
		currentModelIndex: number;
		totalModels: number;
	}>();
</script>

<h2>Provisioning Intelligence Engine</h2>
<p class="status-mono mono">{statusText}</p>

<div class="progress-bar-wrap">
	<div class="progress-fill" style="width: {progressWidth}%"></div>
</div>

<div class="sys-reqs">
	<span>{currentModelName}</span>
	<span>{overallProgress}%</span>
</div>

<div class="dl-stats">
	{#if speedMbps !== null && speedMbps > 0}
		<span>{speedMbps.toFixed(2)} MB/s</span>
	{:else}
		<span>...</span>
	{/if}
	{#if etaSeconds !== null && etaSeconds > 0}
		<span>ETA: {Math.floor(etaSeconds / 60)}m {etaSeconds % 60}s</span>
	{/if}
</div>

<div class="step-counter">
	<span>Model {currentModelIndex} of {totalModels}</span>
</div>

<style>
	h2 {
		font-size: 20px;
		margin-bottom: 8px;
		color: var(--text-primary);
		letter-spacing: 0.05em;
	}

	p {
		font-size: 14px;
		color: var(--text-secondary);
		margin-bottom: 32px;
	}

	.status-mono {
		font-size: 11px;
		color: var(--accent-success);
		margin-bottom: 24px;
		text-transform: uppercase;
		letter-spacing: 0.1em;
	}

	.progress-bar-wrap {
		width: 100%;
		height: 6px;
		background: rgba(255, 255, 255, 0.05);
		border-radius: 3px;
		overflow: hidden;
		margin-bottom: 16px;
		border: 1px solid rgba(255, 255, 255, 0.02);
	}

	.progress-fill {
		height: 100%;
		background: linear-gradient(90deg, var(--accent-primary), #f5d547);
		box-shadow: 0 0 15px var(--accent-primary);
		transition: width 0.2s cubic-bezier(0.25, 0.46, 0.45, 0.94);
	}

	.sys-reqs {
		display: flex;
		justify-content: space-between;
		width: 100%;
		font-size: 10px;
		color: var(--text-tertiary);
		text-transform: uppercase;
		letter-spacing: 0.1em;
		font-weight: 700;
	}

	.dl-stats {
		display: flex;
		justify-content: space-between;
		width: 100%;
		font-size: 10px;
		color: var(--text-tertiary);
		margin-top: 6px;
		font-family: var(--font-mono);
	}

	.step-counter {
		margin-top: 16px;
		font-size: 9px;
		color: var(--text-tertiary);
		text-transform: uppercase;
		letter-spacing: 0.15em;
		font-weight: 600;
	}
</style>
