<script lang="ts">
	import { CheckCircle } from 'lucide-svelte';

	let { statusText } = $props<{ statusText: string }>();
</script>

<h2>Systems Ready</h2>
<p class="status-mono mono">{statusText}</p>
<div class="ready-check">
	<CheckCircle size={48} class="accent-success" />
</div>

<style>
	h2 {
		font-size: var(--text-3xl);
		margin-bottom: var(--space-md);
		color: var(--text-primary);
		letter-spacing: 0.05em;
	}

	p {
		font-size: var(--text-lg);
		color: var(--text-secondary);
		margin-bottom: var(--space-7xl);
	}

	.status-mono {
		font-size: var(--text-sm);
		color: var(--accent-success);
		margin-bottom: var(--space-5xl);
		text-transform: uppercase;
		letter-spacing: 0.1em;
	}

	.ready-check {
		margin-top: var(--space-4xl);
		color: var(--accent-success);
		animation: scale-in 0.5s cubic-bezier(0.175, 0.885, 0.32, 1.275);
	}

	@keyframes scale-in {
		from {
			transform: scale(0);
			opacity: 0;
		}
		to {
			transform: scale(1);
			opacity: 1;
		}
	}
</style>
