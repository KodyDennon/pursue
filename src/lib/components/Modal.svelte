<script lang="ts">
	import type { Snippet } from 'svelte';

	// Shared overlay chrome for the app's modal-like surfaces (AnalysisModal, IntelligenceModal,
	// MediaViewer, GlobalSearch). Previously each was hand-rolled independently: two had no
	// Escape/backdrop-dismiss at all, and z-index/blur/background varied with no shared reason
	// why. This component owns the *behavior* (Escape key, click-outside-to-dismiss) and base
	// overlay positioning; each consumer keeps its own panel markup/styling as slotted content,
	// and can override z-index/background/blur via props to match its existing look exactly.
	let {
		isOpen = $bindable(false),
		onClose,
		closeOnBackdrop = true,
		closeOnEscape = true,
		zIndex = 2000,
		background = 'rgba(0, 0, 0, 0.85)',
		blur = '10px',
		padding = '40px',
		children
	} = $props<{
		isOpen: boolean;
		onClose?: () => void;
		closeOnBackdrop?: boolean;
		closeOnEscape?: boolean;
		zIndex?: number;
		background?: string;
		blur?: string;
		padding?: string;
		children: Snippet;
	}>();

	function close() {
		if (!isOpen) return;
		isOpen = false;
		onClose?.();
	}

	function handleKeydown(e: KeyboardEvent) {
		if (closeOnEscape && e.key === 'Escape') close();
	}

	function handleBackdropClick(e: MouseEvent) {
		if (closeOnBackdrop && e.target === e.currentTarget) close();
	}
</script>

<svelte:window onkeydown={handleKeydown} />

{#if isOpen}
	<div
		class="modal-overlay"
		role="presentation"
		style="z-index: {zIndex}; background: {background}; backdrop-filter: blur({blur}); -webkit-backdrop-filter: blur({blur}); padding: {padding};"
		onclick={handleBackdropClick}
	>
		{@render children()}
	</div>
{/if}

<style>
	.modal-overlay {
		position: fixed;
		inset: 0;
		display: flex;
		align-items: center;
		justify-content: center;
	}
</style>
