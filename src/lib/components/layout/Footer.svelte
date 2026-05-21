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
			>{intelligenceStore.status?.local_records || 0} / {intelligenceStore.status?.official_records || 0} Assets</span
		>
	</div>
	<div class="f-section">
		<span class="f-label">Analysis:</span>
		<span class="f-val">{intelligenceStore.status?.analyzed_records || 0} Reports</span>
	</div>
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
		<span class="f-val"
			>{busy ? `AGENT ${busy.toUpperCase()} ACTIVE` : 'INTELLIGENCE OS STANDBY'}</span
		>
	</div>
</footer>

<style>
	.os-footer {
		height: 32px;
		background: #050608;
		border-top: 1px solid var(--border-subtle);
		display: flex;
		align-items: center;
		padding: 0 32px;
		gap: 32px;
		font-size: 10px;
		letter-spacing: 0.1em;
		color: var(--text-tertiary);
		text-transform: uppercase;
		width: 100%;
		box-sizing: border-box;
	}

	.f-section {
		display: flex;
		gap: 8px;
		align-items: center;
	}

	.resource-monitor {
		margin-left: auto;
		gap: 24px;
		padding-right: 24px;
		border-right: 1px solid var(--border-subtle);
		height: 100%;
	}

	.res-item {
		display: flex;
		align-items: center;
		gap: 12px;
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
		background: var(--accent-primary);
		transition: width 0.3s ease;
	}

	.f-label {
		opacity: 0.5;
	}

	.f-val {
		color: var(--text-secondary);
		font-weight: 600;
	}

	.engine-status {
		margin-left: auto;
		color: var(--accent-primary);
	}

	.status-orb {
		width: 8px;
		height: 8px;
		border-radius: 50%;
		background: #2a2d35;
	}

	.status-orb.busy {
		background: var(--accent-primary);
		box-shadow: 0 0 8px var(--accent-primary);
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
