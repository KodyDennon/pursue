<script lang="ts">
	import { Brain, X } from 'lucide-svelte';

	let { close } = $props<{
		close: () => void;
	}>();
</script>

<header class="panel-header glass-header">
	<div class="brand">
		<Brain size={24} class="accent-icon pulse-active" />
		<div>
			<h2>Cognitive Synthesis Terminal</h2>
			<p>Gemma 4 Deep Intelligence Generation & Forensic Audit</p>
		</div>
	</div>
	<button class="close-btn" onclick={close} aria-label="Close terminal"><X size={20} /></button>
</header>

<style>
	.panel-header {
		padding: 20px 28px;
		display: flex;
		justify-content: space-between;
		align-items: center;
		border-bottom: 1px solid var(--border-subtle);
	}

	.brand {
		display: flex;
		gap: 16px;
		align-items: center;
	}

	.brand h2 {
		margin: 0;
		font-size: 18px;
		font-weight: 600;
		letter-spacing: 0.02em;
	}
	.brand p {
		margin: 2px 0 0 0;
		font-size: 12px;
		color: var(--text-secondary);
	}

	.close-btn {
		background: none;
		border: none;
		color: var(--text-tertiary);
		cursor: pointer;
		padding: 6px;
		border-radius: 50%;
		transition: all 0.2s;
	}

	.close-btn:hover {
		background: rgba(255, 255, 255, 0.05);
		color: #fff;
	}

	:global(.accent-icon) {
		color: var(--accent-primary);
	}

	:global(.pulse-active) {
		animation: pulse-light 1.5s infinite ease-in-out;
	}

	@keyframes pulse-light {
		0%,
		100% {
			opacity: 0.6;
		}
		50% {
			opacity: 1;
			transform: scale(1.05);
		}
	}
</style>
