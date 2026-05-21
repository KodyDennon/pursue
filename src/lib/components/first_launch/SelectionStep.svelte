<script lang="ts">
	import { Cpu, Brain, ChevronRight } from 'lucide-svelte';

	interface HardwareDiagnostics {
		cpu_brand: string;
		total_memory_gb: number;
		recommended_tier: 'Standard' | 'Elite';
	}

	let { specs, selectedTier, onSelectTier, onStartProvisioning } = $props<{
		specs: HardwareDiagnostics | null;
		selectedTier: 'Standard' | 'Elite';
		onSelectTier: (tier: 'Standard' | 'Elite') => void;
		onStartProvisioning: () => void;
	}>();
</script>

<div class="selection-view">
	<h2>Intelligence Tier Selection</h2>
	<p>
		Recommended based on your <strong>{specs?.cpu_brand || 'Processor'}</strong> and
		<strong>{specs?.total_memory_gb || '??'}GB RAM</strong>.
	</p>

	<div class="tier-options">
		<button
			class="tier-card"
			class:active={selectedTier === 'Standard'}
			class:recommended={specs?.recommended_tier === 'Standard'}
			onclick={() => onSelectTier('Standard')}
		>
			<div class="tier-head">
				<Cpu size={24} />
				<div class="t-title">Standard Intel</div>
			</div>
			<p>
				Gemma 4 E2B + BGE. Optimized effective parameter architecture for workstation
				performance.
			</p>
			<div class="tier-meta">~3.2 GB Storage</div>
		</button>

		<button
			class="tier-card"
			class:active={selectedTier === 'Elite'}
			class:recommended={specs?.recommended_tier === 'Elite'}
			onclick={() => onSelectTier('Elite')}
		>
			<div class="tier-head">
				<Brain size={24} />
				<div class="t-title">Elite Intel</div>
			</div>
			<p>Gemma 4 E4B + BGE. Advanced reasoning with native multimodal capabilities.</p>
			<div class="tier-meta">~5.0 GB Storage</div>
		</button>
	</div>

	<button class="provision-btn" onclick={onStartProvisioning}>
		Initialize Neural OS <ChevronRight size={18} />
	</button>
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

	.selection-view {
		width: 100%;
		display: flex;
		flex-direction: column;
		align-items: center;
	}

	.tier-options {
		display: flex;
		gap: 16px;
		margin-bottom: 40px;
		width: 100%;
	}

	.tier-card {
		flex: 1;
		background: rgba(255, 255, 255, 0.01);
		border: 1px solid var(--border-subtle);
		border-radius: 12px;
		padding: 24px;
		text-align: left;
		cursor: pointer;
		transition: all 0.3s;
		display: flex;
		flex-direction: column;
		gap: 12px;
	}

	.tier-card p {
		font-size: 12px;
		margin: 0;
		line-height: 1.5;
		color: var(--text-tertiary);
	}

	.tier-card.active {
		border-color: var(--accent-primary);
		background: rgba(231, 196, 107, 0.05);
		box-shadow: 0 0 20px rgba(231, 196, 107, 0.1);
	}

	.tier-card.recommended {
		position: relative;
	}
	.tier-card.recommended::before {
		content: 'RECOMMENDED';
		position: absolute;
		top: -10px;
		right: 12px;
		font-size: 8px;
		background: var(--accent-primary);
		color: #000;
		padding: 2px 6px;
		border-radius: 4px;
		font-weight: 800;
	}

	.tier-head {
		display: flex;
		align-items: center;
		gap: 12px;
		color: var(--accent-primary);
	}

	.t-title {
		font-weight: 700;
		font-size: 15px;
		color: var(--text-primary);
	}

	.tier-meta {
		font-size: 10px;
		color: var(--text-tertiary);
		text-transform: uppercase;
		margin-top: auto;
	}

	.provision-btn {
		width: 100%;
		background: var(--accent-primary);
		color: #000;
		border: none;
		border-radius: 8px;
		padding: 16px;
		font-weight: 800;
		font-size: 15px;
		display: flex;
		align-items: center;
		justify-content: center;
		gap: 12px;
		cursor: pointer;
		transition: all 0.3s;
		text-transform: uppercase;
		letter-spacing: 0.1em;
	}

	.provision-btn:hover {
		filter: brightness(1.1);
		transform: translateY(-2px);
	}
</style>
