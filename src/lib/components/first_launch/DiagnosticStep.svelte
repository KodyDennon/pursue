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
		font-size: 20px;
		margin-bottom: 8px;
		color: var(--text-primary);
		letter-spacing: 0.05em;
	}

	.status-mono {
		font-size: 11px;
		color: var(--accent-success);
		margin-bottom: 24px;
		text-transform: uppercase;
		letter-spacing: 0.1em;
	}

	.diagnostic-loader {
		padding: 20px;
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
