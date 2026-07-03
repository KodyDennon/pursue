<script lang="ts">
	import { Cpu } from 'lucide-svelte';

	interface HardwareDiagnosticsType {
		cpu_brand: string;
		total_memory_gb: number;
		gpu_acceleration_available: boolean;
		recommended_tier: 'Standard' | 'Elite';
	}

	let { diagnostics } = $props<{
		diagnostics: HardwareDiagnosticsType | null;
	}>();
</script>

<section class="center-card diagnostics">
	<header>
		<Cpu size={18} />
		<h3>Hardware Diagnostics</h3>
	</header>
	{#if diagnostics}
		<div class="diag-metrics">
			<div class="metric">
				<span>Processor</span>
				<strong>{diagnostics.cpu_brand || 'Generic CPU'}</strong>
			</div>
			<div class="metric">
				<span>Memory Pool</span>
				<strong>{diagnostics.total_memory_gb} GB Total</strong>
			</div>
			<div class="metric">
				<span>Acceleration</span>
				<strong class={diagnostics.gpu_acceleration_available ? 'text-success' : 'text-warning'}>
					{diagnostics.gpu_acceleration_available
						? 'GPU Active (Metal/CUDA)'
						: 'CPU Only (Fallback)'}
				</strong>
			</div>
			<div class="metric">
				<span>Intelligence Tier</span>
				<strong class="tier-badge {diagnostics.recommended_tier}">
					{diagnostics.recommended_tier}
				</strong>
			</div>
		</div>
	{:else}
		<div class="loading-state">Probing hardware...</div>
	{/if}
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

	.diag-metrics {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: var(--space-4xl);
	}

	.metric {
		display: flex;
		flex-direction: column;
		gap: var(--space-xs);
	}

	.metric span {
		font-size: var(--text-sm);
		color: var(--color-text-tertiary);
		text-transform: uppercase;
	}

	.metric strong {
		font-size: 15px;
		color: var(--color-text-primary);
	}

	.text-success {
		color: var(--color-accent-success) !important;
	}
	.text-warning {
		color: var(--color-accent-primary) !important;
	}

	.tier-badge {
		display: inline-block;
		padding: 2px 8px;
		border-radius: var(--radius-xs);
		background: rgba(231, 196, 107, 0.1);
		color: var(--color-accent-primary);
	}

	.loading-state {
		padding: var(--space-4xl);
		text-align: center;
		color: var(--color-text-tertiary);
		font-style: italic;
	}
</style>
