<script lang="ts">
	import { Cpu, Brain, ChevronRight } from 'lucide-svelte';

	interface HardwareDiagnostics {
		cpu_brand: string;
		total_memory_gb: number;
		recommended_tier: 'Standard' | 'Advanced' | 'Elite';
	}

	let {
		specs,
		selectedTier,
		hfToken,
		hfUsername,
		hfAuthStatus,
		hfAuthError,
		hfAuthBusy,
		onSelectTier,
		onHfTokenChange,
		onHfSignIn,
		onOpenGemmaLicense,
		onStartProvisioning
	} = $props<{
		specs: HardwareDiagnostics | null;
		selectedTier: 'Standard' | 'Elite';
		hfToken: string;
		hfUsername: string;
		hfAuthStatus: string;
		hfAuthError: string | null;
		hfAuthBusy: boolean;
		onSelectTier: (tier: 'Standard' | 'Elite') => void;
		onHfTokenChange: (token: string) => void;
		onHfSignIn: () => void;
		onOpenGemmaLicense: () => void;
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
			<p>Gemma 4 E4B QAT Q4_0 + BGE. CUDA or Metal is used first; CPU is the final fallback.</p>
			<div class="tier-meta">~5.4 GB Storage</div>
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
			<p>Gemma 4 E4B QAT Q4_0 + BGE with the local vision-analysis runtime enabled.</p>
			<div class="tier-meta">~5.4 GB plus vision models</div>
		</button>
	</div>

	<div class="hf-auth-panel">
		<div>
			<strong>{hfUsername ? `Hugging Face: ${hfUsername}` : 'Hugging Face model access'}</strong>
			<small
				>Sign in through the system browser. PURSUE uses the token only to read gated model files.</small
			>
		</div>
		<button class="hf-signin" disabled={hfAuthBusy} onclick={onHfSignIn}>
			{hfAuthBusy
				? 'Waiting for approval…'
				: hfUsername
					? 'Sign in again'
					: 'Sign in with Hugging Face'}
		</button>
		{#if hfAuthStatus}<p class="auth-status">{hfAuthStatus}</p>{/if}
		{#if hfAuthError}<p class="auth-error" role="alert">{hfAuthError}</p>{/if}
		<button class="license-link" onclick={onOpenGemmaLicense}
			>Review and accept the Gemma license</button
		>
	</div>

	<details class="token-field">
		<summary>Use a read token manually instead</summary>
		<input
			type="password"
			value={hfToken}
			oninput={(event) => onHfTokenChange(event.currentTarget.value)}
			autocomplete="off"
			spellcheck="false"
			placeholder="hf_…"
		/>
		<small>
			The token is stored locally and is never written to logs. HF_TOKEN remains supported for
			automated installations.
		</small>
	</details>

	<button class="provision-btn" onclick={onStartProvisioning}>
		Initialize Neural OS <ChevronRight size={18} />
	</button>
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

	.selection-view {
		width: 100%;
		display: flex;
		flex-direction: column;
		align-items: center;
	}

	.tier-options {
		display: flex;
		gap: var(--space-3xl);
		margin-bottom: var(--space-8xl);
		width: 100%;
	}

	.tier-card {
		flex: 1;
		background: rgba(255, 255, 255, 0.01);
		border: 1px solid var(--color-border-subtle);
		border-radius: var(--radius-md);
		padding: var(--space-5xl);
		text-align: left;
		cursor: pointer;
		transition: all 0.3s;
		display: flex;
		flex-direction: column;
		gap: var(--space-xl);
	}

	.tier-card p {
		font-size: var(--text-base);
		margin: 0;
		line-height: 1.5;
		color: var(--color-text-tertiary);
	}

	.tier-card.active {
		border-color: var(--color-accent-primary);
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
		background: var(--color-accent-primary);
		color: #000;
		padding: 2px 6px;
		border-radius: var(--radius-xs);
		font-weight: 800;
	}

	.tier-head {
		display: flex;
		align-items: center;
		gap: var(--space-xl);
		color: var(--color-accent-primary);
	}

	.t-title {
		font-weight: 700;
		font-size: 15px;
		color: var(--color-text-primary);
	}

	.tier-meta {
		font-size: var(--text-xs);
		color: var(--color-text-tertiary);
		text-transform: uppercase;
		margin-top: auto;
	}

	.token-field {
		width: 100%;
		text-align: left;
		margin-bottom: var(--space-5xl);
		color: var(--color-text-secondary);
		font-size: var(--text-sm);
	}

	.hf-auth-panel {
		box-sizing: border-box;
		width: 100%;
		margin-bottom: var(--space-3xl);
		padding: var(--space-3xl);
		border: 1px solid var(--color-border-subtle);
		border-radius: var(--radius-base);
		background: rgba(255, 255, 255, 0.02);
		text-align: left;
	}

	.hf-auth-panel strong,
	.hf-auth-panel small {
		display: block;
	}

	.hf-auth-panel small {
		margin-top: var(--space-xs);
		color: var(--color-text-tertiary);
		line-height: 1.4;
	}

	.hf-signin {
		width: 100%;
		margin-top: var(--space-xl);
		padding: var(--space-xl);
		border: 1px solid var(--color-accent-primary);
		border-radius: var(--radius-base);
		background: transparent;
		color: var(--color-accent-primary);
		font-weight: 700;
		cursor: pointer;
	}

	.hf-signin:disabled {
		opacity: 0.65;
		cursor: wait;
	}

	.auth-status,
	.auth-error {
		margin: var(--space-xl) 0 0;
		font-size: var(--text-sm);
		line-height: 1.4;
	}

	.auth-status {
		color: var(--color-accent-success);
	}

	.auth-error {
		color: var(--color-accent-danger, #e57373);
	}

	.license-link {
		margin-top: var(--space-xl);
		padding: 0;
		border: 0;
		background: transparent;
		color: var(--color-text-secondary);
		text-decoration: underline;
		cursor: pointer;
	}

	.token-field summary {
		cursor: pointer;
	}

	.token-field small {
		display: block;
	}

	.token-field input {
		box-sizing: border-box;
		width: 100%;
		margin: var(--space-sm) 0;
		padding: var(--space-xl);
		border: 1px solid var(--color-border-subtle);
		border-radius: var(--radius-base);
		background: rgba(255, 255, 255, 0.03);
		color: var(--color-text-primary);
		font-family: var(--font-mono);
	}

	.token-field input:focus {
		outline: none;
		border-color: var(--color-accent-primary);
	}

	.token-field small {
		color: var(--color-text-tertiary);
		line-height: 1.4;
	}

	.provision-btn {
		width: 100%;
		background: var(--color-accent-primary);
		color: #000;
		border: none;
		border-radius: var(--radius-base);
		padding: var(--space-3xl);
		font-weight: 800;
		font-size: 15px;
		display: flex;
		align-items: center;
		justify-content: center;
		gap: var(--space-xl);
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
