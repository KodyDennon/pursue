<script lang="ts">
	import { Minus, Square, Copy, X } from 'lucide-svelte';
	import { getCurrentWindow } from '@tauri-apps/api/window';
	import { onMount } from 'svelte';

	let isMaximized = $state(false);

	async function updateMaximizedState() {
		try {
			const appWindow = getCurrentWindow();
			isMaximized = await appWindow.isMaximized();
		} catch {
			// Non-Tauri fallback
		}
	}

	onMount(() => {
		updateMaximizedState();
		const interval = setInterval(updateMaximizedState, 500);
		return () => clearInterval(interval);
	});

	async function minimize() {
		try {
			await getCurrentWindow().minimize();
		} catch {
			// Ignore fallback error
		}
	}

	async function toggleMaximize() {
		try {
			const appWindow = getCurrentWindow();
			await appWindow.toggleMaximize();
			isMaximized = await appWindow.isMaximized();
		} catch {
			// Ignore fallback error
		}
	}

	async function closeWindow() {
		try {
			await getCurrentWindow().close();
		} catch {
			// Ignore fallback error
		}
	}
</script>

<div class="window-controls-container" data-tauri-drag-region={false}>
	<button class="win-ctrl-btn minimize" onclick={minimize} title="Minimize Window">
		<Minus size={14} />
	</button>
	<button
		class="win-ctrl-btn maximize"
		onclick={toggleMaximize}
		title={isMaximized ? 'Restore Window' : 'Maximize Window'}
	>
		{#if isMaximized}
			<Copy size={13} />
		{:else}
			<Square size={13} />
		{/if}
	</button>
	<button class="win-ctrl-btn close" onclick={closeWindow} title="Close App">
		<X size={15} />
	</button>
</div>

<style>
	.window-controls-container {
		display: flex;
		align-items: center;
		height: 100%;
		-webkit-app-region: no-drag;
		user-select: none;
		z-index: 9999;
	}

	.win-ctrl-btn {
		width: 44px;
		height: 32px;
		display: flex;
		align-items: center;
		justify-content: center;
		background: transparent;
		border: none;
		color: rgba(255, 255, 255, 0.7);
		cursor: pointer;
		transition: background-color 0.15s ease, color 0.15s ease;
	}

	.win-ctrl-btn:hover {
		background-color: rgba(255, 255, 255, 0.1);
		color: #fff;
	}

	.win-ctrl-btn.close:hover {
		background-color: #e81123;
		color: #fff;
	}

	.win-ctrl-btn:active {
		background-color: rgba(255, 255, 255, 0.2);
	}

	.win-ctrl-btn.close:active {
		background-color: #f1707a;
	}
</style>
