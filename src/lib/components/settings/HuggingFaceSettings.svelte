<script lang="ts">
	import { ShieldCheck, Save } from 'lucide-svelte';
	import { settingsStore } from '$lib/stores/settingsStore.svelte';
</script>

<section class="settings-section glass-panel">
	<div class="s-header">
		<ShieldCheck size={18} class="accent-icon" />
		<h3>Hugging Face Authentication</h3>
	</div>
	<div class="s-body">
		<p class="section-desc">
			Provide a User Access Token to download gated models (e.g., official Google Gemma series).
			Create one at <a
				href="https://huggingface.co/settings/tokens"
				target="_blank"
				class="accent-link">huggingface.co/settings/tokens</a
			>.
		</p>
		<div class="data-item">
			<span class="d-label">Access Token</span>
			<input
				type="password"
				bind:value={settingsStore.hfToken}
				placeholder="hf_..."
				class="persona-input token-input"
			/>
		</div>
	</div>
	<footer class="s-footer">
		<button
			class="s-btn primary"
			onclick={() => settingsStore.saveHfToken()}
			disabled={settingsStore.busy === 'token'}
		>
			<Save size={14} /> Update Token
		</button>
	</footer>
</section>

<style>
	.settings-section {
		background: var(--color-bg-surface);
		border: 1px solid var(--color-border-subtle);
		border-radius: var(--radius-lg);
		display: flex;
		flex-direction: column;
	}

	.s-header {
		padding: var(--space-5xl);
		display: flex;
		align-items: center;
		gap: var(--space-3xl);
		border-bottom: 1px solid var(--color-border-subtle);
	}

	.s-header h3 {
		font-size: var(--text-xl);
		font-weight: 600;
		margin: 0;
	}

	.s-body {
		padding: var(--space-5xl);
		flex: 1;
		display: flex;
		flex-direction: column;
		gap: var(--space-5xl);
	}

	.section-desc {
		font-size: var(--text-md);
		color: var(--color-text-secondary);
		line-height: 1.6;
		margin: 0;
	}

	.accent-link {
		color: var(--color-accent-primary);
		text-decoration: none;
	}
	.accent-link:hover {
		text-decoration: underline;
	}

	.data-item {
		display: flex;
		flex-direction: column;
		gap: var(--space-md);
	}

	.d-label {
		font-size: var(--text-sm);
		text-transform: uppercase;
		letter-spacing: 0.1em;
		color: var(--color-text-tertiary);
	}

	.persona-input {
		width: 100%;
		background: rgba(0, 0, 0, 0.3);
		border: 1px solid var(--color-border-subtle);
		border-radius: var(--radius-base);
		padding: var(--space-3xl);
		color: var(--color-text-primary);
		font-family: var(--font-mono);
		font-size: var(--text-md);
		outline: none;
		transition: border-color 0.2s;
	}

	.persona-input.token-input {
		min-height: unset;
		resize: none;
	}

	.persona-input:focus {
		border-color: var(--color-accent-primary);
	}

	.s-footer {
		padding: 16px 24px;
		background: rgba(255, 255, 255, 0.02);
		border-top: 1px solid var(--color-border-subtle);
	}

	.s-btn {
		display: flex;
		align-items: center;
		gap: var(--space-md);
		padding: 8px 16px;
		border-radius: var(--radius-sm);
		font-size: var(--text-base);
		font-weight: 700;
		cursor: pointer;
		border: 1px solid transparent;
		transition: all 0.2s;
	}

	.s-btn.primary {
		background: var(--color-accent-primary);
		color: #000;
	}

	.s-btn:hover {
		filter: brightness(1.1);
		transform: translateY(-1px);
	}

	:global(.accent-icon) {
		color: var(--color-accent-primary);
	}
</style>
