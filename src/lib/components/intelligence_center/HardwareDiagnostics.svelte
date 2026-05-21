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
				<strong
					class={diagnostics.gpu_acceleration_available ? 'text-success' : 'text-warning'}
				>
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

	.diag-metrics {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: 20px;
	}

	.metric {
		display: flex;
		flex-direction: column;
		gap: 4px;
	}

	.metric span {
		font-size: 11px;
		color: var(--text-tertiary);
		text-transform: uppercase;
	}

	.metric strong {
		font-size: 15px;
		color: var(--text-primary);
	}

	.text-success {
		color: var(--accent-success) !important;
	}
	.text-warning {
		color: #f3c46b !important;
	}

	.tier-badge {
		display: inline-block;
		padding: 2px 8px;
		border-radius: 4px;
		background: rgba(231, 196, 107, 0.1);
		color: var(--accent-primary);
	}

	.loading-state {
		padding: 20px;
		text-align: center;
		color: var(--text-tertiary);
		font-style: italic;
	}
</style>
