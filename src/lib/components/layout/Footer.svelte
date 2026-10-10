<script lang="ts">
	import { formatBytes } from '$lib/utils';
	import { intelligenceStore } from '$lib/stores/intelligenceStore.svelte';

	let { systemStats, busy } = $props<{
		systemStats: { cpu_usage: number; process_memory_mb: number } | null;
		busy: string | null;
	}>();
</script>

<footer class="os-footer">
	<div class="f-section">
		<span class="f-label">Ingestion:</span>
		<span class="f-val"
			>{intelligenceStore.status?.local_records || 0} / {intelligenceStore.status
				?.official_records || 0} Assets</span
		>
	</div>
	<div class="f-section">
		<span class="f-label">Analysis:</span>
		<span class="f-val">{intelligenceStore.status?.analyzed_records || 0} Reports</span>
	</div>
	<div class="f-cluster">
		<div class="f-section resource-monitor">
			{#if systemStats}
				<div class="res-item">
					<span class="f-label">CPU</span>
					<div class="res-bar-wrap">
						<div class="res-bar-fill" style="width: {systemStats.cpu_usage}%"></div>
					</div>
					<span class="f-val">{systemStats.cpu_usage.toFixed(1)}%</span>
				</div>
				<div class="res-item">
					<span class="f-label">MEM</span>
					<span class="f-val">{formatBytes(systemStats.process_memory_mb * 1024 * 1024)}</span>
				</div>
			{/if}
		</div>

		<div class="f-section engine-status">
			<div class="status-orb" class:busy></div>
			<span class="f-val">{busy ? `${busy} active` : 'Standby'}</span>
		</div>
	</div>
</footer>

<style>
	.os-footer {
		height: 36px;
		flex-shrink: 0;
		background: #050608;
		border-top: 1px solid var(--color-border-subtle);
		display: flex;
		align-items: center;
		padding: 0 16px;
		gap: var(--space-4xl);
		font-size: var(--text-xs);
		letter-spacing: 0.04em;
		color: var(--color-text-tertiary);
		text-transform: uppercase;
		width: 100%;
		min-width: 0;
		box-sizing: border-box;
		overflow: hidden;
	}

	.f-section {
		display: flex;
		gap: var(--space-md);
		align-items: center;
		flex-shrink: 0;
		white-space: nowrap;
	}

	.f-cluster {
		margin-left: auto;
		display: flex;
		align-items: center;
		gap: var(--space-3xl);
		min-width: 0;
		flex-shrink: 1;
	}

	.resource-monitor {
		gap: var(--space-3xl);
		padding-right: var(--space-3xl);
		border-right: 1px solid var(--color-border-subtle);
		height: 100%;
	}

	.res-item {
		display: flex;
		align-items: center;
		gap: var(--space-xl);
	}

	.res-bar-wrap {
		width: 40px;
		height: 3px;
		background: rgba(255, 255, 255, 0.05);
		border-radius: 1px;
		overflow: hidden;
	}

	.res-bar-fill {
		height: 100%;
		background: var(--color-accent-primary);
		transition: width 0.3s ease;
	}

	.f-label {
		opacity: 0.5;
	}

	.f-val {
		color: var(--color-text-secondary);
		font-weight: 600;
	}

	.engine-status {
		color: var(--color-accent-primary);
		min-width: 0;
		flex-shrink: 1;
	}

	.engine-status .f-val {
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.status-orb {
		width: 8px;
		height: 8px;
		border-radius: 50%;
		background: #2a2d35;
	}

	.status-orb.busy {
		background: var(--color-accent-primary);
		box-shadow: 0 0 8px var(--color-accent-primary);
		animation: orb-pulse 2s infinite;
	}

	@keyframes orb-pulse {
		0% {
			opacity: 1;
			transform: scale(1);
		}
		50% {
			opacity: 0.5;
			transform: scale(1.2);
		}
		100% {
			opacity: 1;
			transform: scale(1);
		}
	}
</style>
