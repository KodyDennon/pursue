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
		background: var(--bg-surface);
		border: 1px solid var(--border-subtle);
		border-radius: var(--radius-lg);
		display: flex;
		flex-direction: column;
	}

	.settings-section.full-width {
		grid-column: 1 / -1;
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

	.persona-input {
		width: 100%;
		min-height: 120px;
		background: rgba(0, 0, 0, 0.3);
		border: 1px solid var(--border-subtle);
		border-radius: 8px;
		padding: 16px;
		color: var(--text-primary);
		font-family: var(--font-mono);
		font-size: 13px;
		resize: vertical;
		outline: none;
		transition: border-color 0.2s;
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
