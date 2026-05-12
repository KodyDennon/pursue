<script lang="ts">
	import { toasts, removeToast } from '$lib/toastStore';
	import { Info, AlertCircle, CheckCircle2, Loader2, X } from 'lucide-svelte';
	import { fade, fly } from 'svelte/transition';
</script>

<div class="toast-container">
	{#each $toasts as toast (toast.id)}
		<div
			class="toast-card {toast.type}"
			in:fly={{ y: 20, duration: 300 }}
			out:fade={{ duration: 200 }}
		>
			<div class="toast-icon">
				{#if toast.type === 'success'}
					<CheckCircle2 size={18} />
				{:else if toast.type === 'error'}
					<AlertCircle size={18} />
				{:else if toast.type === 'info'}
					<Info size={18} />
				{:else if toast.type === 'loading'}
					<Loader2 size={18} class="spin" />
				{/if}
			</div>
			<div class="toast-message">{toast.message}</div>
			{#if toast.type !== 'loading'}
				<button class="toast-close" onclick={() => removeToast(toast.id)}>
					<X size={14} />
				</button>
			{/if}
		</div>
	{/each}
</div>

<style>
	.toast-container {
		position: fixed;
		bottom: 24px;
		right: 24px;
		display: flex;
		flex-direction: column;
		gap: 12px;
		z-index: 9999;
		pointer-events: none;
	}

	.toast-card {
		display: flex;
		align-items: center;
		gap: 12px;
		background: var(--bg-surface-elevated);
		border: 1px solid var(--border-subtle);
		padding: 12px 16px;
		border-radius: var(--radius-md);
		box-shadow: 0 8px 24px rgba(0, 0, 0, 0.4);
		min-width: 300px;
		max-width: 400px;
		pointer-events: auto;
	}

	.toast-card.success {
		border-left: 3px solid var(--accent-success);
	}

	.toast-card.error {
		border-left: 3px solid var(--accent-danger);
	}

	.toast-card.info {
		border-left: 3px solid var(--accent-primary);
	}

	.toast-card.loading {
		border-left: 3px solid var(--text-secondary);
	}

	.toast-icon {
		display: flex;
		align-items: center;
		justify-content: center;
	}

	.toast-card.success .toast-icon {
		color: var(--accent-success);
	}
	.toast-card.error .toast-icon {
		color: var(--accent-danger);
	}
	.toast-card.info .toast-icon {
		color: var(--accent-primary);
	}
	.toast-card.loading .toast-icon {
		color: var(--text-secondary);
	}

	.toast-message {
		flex: 1;
		font-size: 14px;
		color: var(--text-primary);
		line-height: 1.4;
	}

	.toast-close {
		color: var(--text-secondary);
		background: transparent;
		border: none;
		cursor: pointer;
		padding: 4px;
		display: flex;
		align-items: center;
		justify-content: center;
		border-radius: 4px;
		transition: var(--transition-fast);
	}

	.toast-close:hover {
		background: rgba(255, 255, 255, 0.1);
		color: var(--text-primary);
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
