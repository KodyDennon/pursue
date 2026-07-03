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
		font-size: var(--text-3xl);
		margin-bottom: var(--space-md);
		color: var(--color-text-primary);
		letter-spacing: 0.05em;
	}

	p {
		font-size: var(--text-lg);
		color: var(--color-text-secondary);
		margin-bottom: var(--space-7xl);
	}

	.status-mono {
		font-size: var(--text-sm);
		color: var(--color-accent-success);
		margin-bottom: var(--space-5xl);
		text-transform: uppercase;
		letter-spacing: 0.1em;
	}

	.progress-bar-wrap {
		width: 100%;
		height: 6px;
		background: rgba(255, 255, 255, 0.05);
		border-radius: 3px;
		overflow: hidden;
		margin-bottom: var(--space-3xl);
		border: 1px solid rgba(255, 255, 255, 0.02);
	}

	.progress-fill {
		height: 100%;
		background: linear-gradient(90deg, var(--color-accent-primary), #f5d547);
		box-shadow: 0 0 15px var(--color-accent-primary);
		transition: width 0.2s cubic-bezier(0.25, 0.46, 0.45, 0.94);
	}

	.sys-reqs {
		display: flex;
		justify-content: space-between;
		width: 100%;
		font-size: var(--text-xs);
		color: var(--color-text-tertiary);
		text-transform: uppercase;
		letter-spacing: 0.1em;
		font-weight: 700;
	}

	.dl-stats {
		display: flex;
		justify-content: space-between;
		width: 100%;
		font-size: var(--text-xs);
		color: var(--color-text-tertiary);
		margin-top: var(--space-sm);
		font-family: var(--font-mono);
	}

	.step-counter {
		margin-top: var(--space-3xl);
		font-size: var(--text-2xs);
		color: var(--color-text-tertiary);
		text-transform: uppercase;
		letter-spacing: 0.15em;
		font-weight: 600;
	}
</style>
