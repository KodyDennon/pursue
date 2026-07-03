<script lang="ts">
	import { Brain, Save } from 'lucide-svelte';
	import { settingsStore } from '$lib/stores/settingsStore.svelte';
</script>

<section class="settings-section glass-panel full-width">
	<div class="s-header">
		<Brain size={18} class="accent-icon" />
		<h3>Intelligence Persona Directive</h3>
	</div>
	<div class="s-body">
		<p class="section-desc">
			Inject custom instructions into Gemma 4's core reasoning engine. This modifier is applied to
			all forensic extractions.
		</p>
		<textarea
			bind:value={settingsStore.personaModifier}
			placeholder="e.g., 'Focus heavily on technical sensor data and skepticism regarding atmospheric phenomena...'"
			class="persona-input"
		></textarea>
	</div>
	<footer class="s-footer">
		<button
			class="s-btn primary"
			onclick={() => settingsStore.savePersona()}
			disabled={settingsStore.busy === 'persona'}
		>
			<Save size={14} /> Update Core Directive
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

	.settings-section.full-width {
		grid-column: 1 / -1;
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

	.persona-input {
		width: 100%;
		min-height: 120px;
		background: rgba(0, 0, 0, 0.3);
		border: 1px solid var(--color-border-subtle);
		border-radius: var(--radius-base);
		padding: var(--space-3xl);
		color: var(--color-text-primary);
		font-family: var(--font-mono);
		font-size: var(--text-md);
		resize: vertical;
		outline: none;
		transition: border-color 0.2s;
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
