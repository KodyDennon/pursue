<script lang="ts">
	import { Database, Download, CheckCircle2, Loader2 } from 'lucide-svelte';

	interface IntelligenceModel {
		id: string;
		name: string;
		type: string;
		size: string;
		status: string;
		progress: number;
		url: string;
		filename: string;
		speedMbps: number | null;
		etaSeconds: number | null;
	}

	let {
		models,
		runtimeProvisioned,
		runtimeBusy,
		busyModelId,
		onProvisionRuntime,
		onDownloadModel,
		onProvisionAll
	} = $props<{
		models: IntelligenceModel[];
		runtimeProvisioned: boolean;
		runtimeBusy: boolean;
		busyModelId: string | null;
		onProvisionRuntime: () => void;
		onDownloadModel: (modelId: string) => void;
		onProvisionAll: () => void;
	}>();
</script>

<section class="center-card models">
	<header>
		<Database size={18} />
		<div class="header-content">
			<h3>Cognitive Models</h3>
			{#if models.some((m: IntelligenceModel) => m.status === 'missing')}
				<button class="text-btn" onclick={onProvisionAll} disabled={!!busyModelId}>
					<Download size={14} /> Provision All Missing
				</button>
			{/if}
		</div>
	</header>
	<div class="model-list">
		<!-- Neural Vision Runtime (Python) -->
		<div class="model-item" class:busy={runtimeBusy}>
			<div class="model-info">
				<span class="m-type">Neural Engine</span>
				<span class="m-name">Neural Vision Runtime (Python)</span>
				<span class="m-size">~150 MB • {runtimeProvisioned ? 'ready' : runtimeBusy ? 'provisioning' : 'missing'}</span>
			</div>
			<div class="model-actions">
				{#if runtimeBusy}
					<Loader2 class="spin" size={18} />
				{:else if runtimeProvisioned}
					<CheckCircle2 class="text-success" size={18} />
				{:else}
					<button class="icon-btn" onclick={onProvisionRuntime}>
						<Download size={18} />
					</button>
				{/if}
			</div>
		</div>

		{#each models as model (model.id)}
			<div class="model-item" class:busy={busyModelId === model.id}>
				<div class="model-info">
					<span class="m-type">{model.type}</span>
					<span class="m-name">{model.name}</span>
					{#if model.status === 'downloading'}
						<div class="progress-container">
							<div class="progress-bar" style="width: {model.progress}%"></div>
							<div class="m-stats">
								<span class="m-size">{model.progress.toFixed(1)}% of {model.size}</span>
								<span class="m-eta">
									{#if model.speedMbps !== null && model.speedMbps > 0}
										{model.speedMbps.toFixed(2)} MB/s
									{:else}
										...
									{/if}
									{#if model.etaSeconds !== null}
										• ETA: {model.etaSeconds}s
									{/if}
								</span>
							</div>
						</div>
					{:else}
						<span class="m-size">{model.size} • {model.status}</span>
					{/if}
				</div>
				<div class="model-actions">
					{#if busyModelId === model.id}
						<Loader2 class="spin" size={18} />
					{:else if model.status === 'ready'}
						<CheckCircle2 class="text-success" size={18} />
					{:else}
						<button class="icon-btn" onclick={() => onDownloadModel(model.id)}>
							<Download size={18} />
						</button>
					{/if}
				</div>
			</div>
		{/each}
	</div>
</section>

<style>
	.center-card {
		background: var(--bg-surface);
		border: 1px solid var(--border-subtle);
		border-radius: var(--radius-lg);
		padding: 24px;
		display: flex;
		flex-direction: column;
		gap: 20px;
	}

	.center-card header {
		display: flex;
		align-items: center;
		gap: 12px;
		color: var(--text-secondary);
	}

	.center-card h3 {
		margin: 0;
		font-size: 14px;
		text-transform: uppercase;
		letter-spacing: 0.1em;
		font-weight: 700;
		flex: 1;
	}

	.header-content {
		display: flex;
		align-items: center;
		gap: 12px;
		width: 100%;
	}

	.text-btn {
		background: none;
		border: none;
		color: var(--accent-primary);
		font-size: 11px;
		font-weight: 700;
		text-transform: uppercase;
		display: flex;
		align-items: center;
		gap: 6px;
		cursor: pointer;
		padding: 4px 8px;
		border-radius: 4px;
		transition: background 0.2s;
	}

	.text-btn:hover {
		background: rgba(231, 196, 107, 0.1);
	}

	.text-btn:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	.model-list {
		display: flex;
		flex-direction: column;
		gap: 12px;
	}

	.model-item {
		display: flex;
		justify-content: space-between;
		align-items: center;
		padding: 16px;
		background: rgba(255, 255, 255, 0.02);
		border: 1px solid var(--border-subtle);
		border-radius: var(--radius-md);
		transition: var(--transition-fast);
	}

	.model-item.busy {
		border-color: var(--accent-primary);
		background: rgba(231, 196, 107, 0.05);
	}

	.progress-container {
		margin-top: 8px;
		width: 200px;
		height: 4px;
		background: rgba(255, 255, 255, 0.05);
		border-radius: 2px;
		position: relative;
		overflow: hidden;
	}

	.progress-bar {
		height: 100%;
		background: var(--accent-primary);
		box-shadow: 0 0 8px var(--accent-primary);
		transition: width 0.2s ease;
	}

	.model-info {
		display: flex;
		flex-direction: column;
		gap: 2px;
	}

	.m-type {
		font-size: 10px;
		text-transform: uppercase;
		color: var(--text-tertiary);
	}
	.m-name {
		font-size: 14px;
		font-weight: 600;
		color: var(--text-primary);
	}
	.m-size {
		font-size: 12px;
		color: var(--text-secondary);
	}

	.m-stats {
		display: flex;
		justify-content: space-between;
		margin-top: 4px;
		font-size: 11px;
	}

	.m-eta {
		color: var(--text-tertiary);
		font-family: var(--font-mono);
	}

	.icon-btn {
		background: none;
		border: none;
		width: 32px;
		height: 32px;
		display: flex;
		align-items: center;
		justify-content: center;
		border-radius: 50%;
		color: var(--text-secondary);
		cursor: pointer;
		transition: var(--transition-fast);
	}

	.icon-btn:hover {
		background: var(--bg-surface-elevated);
		color: var(--accent-primary);
	}

	:global(.text-success) {
		color: var(--accent-success) !important;
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
