<script lang="ts">
	import { Loader2 } from 'lucide-svelte';

	let { statusText } = $props<{ statusText: string }>();
</script>

<h2>Black-Ops Initialization</h2>
<p class="status-mono mono">{statusText}</p>
<div class="diagnostic-loader">
	<Loader2 size={24} class="spin" />
</div>

<style>
	h2 {
		font-size: var(--text-3xl);
		margin-bottom: var(--space-md);
		color: var(--text-primary);
		letter-spacing: 0.05em;
	}

	.status-mono {
		font-size: var(--text-sm);
		color: var(--accent-success);
		margin-bottom: var(--space-5xl);
		text-transform: uppercase;
		letter-spacing: 0.1em;
	}

	.diagnostic-loader {
		padding: var(--space-4xl);
		color: var(--accent-primary);
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
