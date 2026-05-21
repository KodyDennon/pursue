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
		background: var(--bg-surface);
		border: 1px solid var(--border-subtle);
		border-radius: var(--radius-lg);
		display: flex;
		flex-direction: column;
	}

	.s-header {
		padding: 24px;
		display: flex;
		align-items: center;
		gap: 16px;
		border-bottom: 1px solid var(--border-subtle);
	}

	.s-header h3 {
		font-size: 16px;
		font-weight: 600;
		margin: 0;
	}

	.s-body {
		padding: 24px;
		flex: 1;
		display: flex;
		flex-direction: column;
		gap: 24px;
	}

	.section-desc {
		font-size: 13px;
		color: var(--text-secondary);
		line-height: 1.6;
		margin: 0;
	}

	.accent-link {
		color: var(--accent-primary);
		text-decoration: none;
	}
	.accent-link:hover {
		text-decoration: underline;
	}

	.data-item {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}

	.d-label {
		font-size: 11px;
		text-transform: uppercase;
		letter-spacing: 0.1em;
		color: var(--text-tertiary);
	}

	.persona-input {
		width: 100%;
		background: rgba(0, 0, 0, 0.3);
		border: 1px solid var(--border-subtle);
		border-radius: 8px;
		padding: 16px;
		color: var(--text-primary);
		font-family: var(--font-mono);
		font-size: 13px;
		outline: none;
		transition: border-color 0.2s;
	}

	.persona-input.token-input {
		min-height: unset;
		resize: none;
	}

	.persona-input:focus {
		border-color: var(--accent-primary);
	}

	.s-footer {
		padding: 16px 24px;
		background: rgba(255, 255, 255, 0.02);
		border-top: 1px solid var(--border-subtle);
	}

	.s-btn {
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 8px 16px;
		border-radius: var(--radius-sm);
		font-size: 12px;
		font-weight: 700;
		cursor: pointer;
		border: 1px solid transparent;
		transition: all 0.2s;
	}

	.s-btn.primary {
		background: var(--accent-primary);
		color: #000;
	}

	.s-btn:hover {
		filter: brightness(1.1);
		transform: translateY(-1px);
	}

	:global(.accent-icon) {
		color: var(--accent-primary);
	}
</style>
