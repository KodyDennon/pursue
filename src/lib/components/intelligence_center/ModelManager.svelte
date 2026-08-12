<script lang="ts">
	import { Database, Download, CheckCircle2, Loader2 } from 'lucide-svelte';

	interface IntelligenceModel {
		id: string;
		name: string;
		type: string;
		size: string;
		status: string;
		progress: number;
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
				<span class="m-size"
					>~150 MB • {runtimeProvisioned ? 'ready' : runtimeBusy ? 'provisioning' : 'missing'}</span
				>
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
						<div class="model-progress-block">
							<div class="progress-bar-track">
								<div class="progress-bar-fill" style="width: {model.progress}%"></div>
							</div>
							<div class="m-stats">
								<span class="m-size">{model.progress.toFixed(1)}% of {model.size}</span>
								<span class="m-eta">
									{#if model.speedMbps !== null && model.speedMbps > 0}
										{model.speedMbps.toFixed(2)} MB/s
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
		background: var(--color-bg-surface);
		border: 1px solid var(--color-border-subtle);
		border-radius: var(--radius-lg);
		padding: var(--space-5xl);
		display: flex;
		flex-direction: column;
		gap: var(--space-4xl);
	}

	.center-card header {
		display: flex;
		align-items: center;
		gap: var(--space-xl);
		color: var(--color-text-secondary);
	}

	.center-card h3 {
		margin: 0;
		font-size: var(--text-lg);
		text-transform: uppercase;
		letter-spacing: 0.1em;
		font-weight: 700;
		flex: 1;
	}

	.header-content {
		display: flex;
		align-items: center;
		gap: var(--space-xl);
		width: 100%;
	}

	.text-btn {
		background: none;
		border: none;
		color: var(--color-accent-primary);
		font-size: var(--text-sm);
		font-weight: 700;
		text-transform: uppercase;
		display: flex;
		align-items: center;
		gap: var(--space-sm);
		cursor: pointer;
		padding: 4px 8px;
		border-radius: var(--radius-xs);
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
		gap: var(--space-xl);
	}

	.model-item {
		display: flex;
		justify-content: space-between;
		align-items: center;
		padding: var(--space-3xl);
		background: rgba(255, 255, 255, 0.02);
		border: 1px solid var(--color-border-subtle);
		border-radius: var(--radius-md);
		transition: var(--transition-fast);
	}

	.model-item.busy {
		border-color: var(--color-accent-primary);
		background: rgba(231, 196, 107, 0.05);
	}

	.model-progress-block {
		margin-top: var(--space-xs);
		width: 240px;
		display: flex;
		flex-direction: column;
		gap: 4px;
	}

	.progress-bar-track {
		width: 100%;
		height: 4px;
		background: rgba(255, 255, 255, 0.08);
		border-radius: var(--radius-2xs);
		position: relative;
		overflow: hidden;
	}

	.progress-bar-fill {
		height: 100%;
		background: var(--color-accent-primary);
		box-shadow: 0 0 8px var(--color-accent-primary);
		transition: width 0.2s ease;
	}

	.model-info {
		display: flex;
		flex-direction: column;
		gap: var(--space-2xs);
	}

	.m-type {
		font-size: var(--text-xs);
		text-transform: uppercase;
		color: var(--color-text-tertiary);
	}
	.m-name {
		font-size: var(--text-lg);
		font-weight: 600;
		color: var(--color-text-primary);
	}
	.m-size {
		font-size: var(--text-base);
		color: var(--color-text-secondary);
	}

	.m-stats {
		display: flex;
		justify-content: space-between;
		margin-top: var(--space-xs);
		font-size: var(--text-sm);
	}

	.m-eta {
		color: var(--color-text-tertiary);
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
		color: var(--color-text-secondary);
		cursor: pointer;
		transition: var(--transition-fast);
	}

	.icon-btn:hover {
		background: var(--color-bg-surface-elevated);
		color: var(--color-accent-primary);
	}

	:global(.text-success) {
		color: var(--color-accent-success) !important;
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
