<script lang="ts">
	import { logger } from '$lib/logger';
	logger.debug('[Layout] Script initializing...');
	import '../app.css';
	import { onMount } from 'svelte';
	import AppDock from '$lib/components/AppDock.svelte';
	import GlobalSearch from '$lib/components/GlobalSearch.svelte';
	import Toasts from '$lib/components/Toasts.svelte';
	import WindowControls from '$lib/components/WindowControls.svelte';
	import { settingsStore } from '$lib/stores/settingsStore.svelte';

	let { children } = $props();

	onMount(() => {
		logger.debug('[Layout] Layout mounted.');
	});

	$effect(() => {
		if (typeof document !== 'undefined') {
			document.documentElement.classList.toggle('performance-mode', settingsStore.performanceMode);
		}
	});
</script>

<div class="app-layout">
	<AppDock />
	<div class="app-content-wrapper">
		<header class="app-topbar" data-tauri-drag-region>
			<div class="app-title-drag" data-tauri-drag-region>
				<span class="app-title">PURSUE Data Analyzer</span>
			</div>
			<WindowControls />
		</header>
		<main class="app-main" data-tauri-drag-region={false}>
			{@render children()}
		</main>
	</div>
	<GlobalSearch />
	<Toasts />
</div>

<style>
	.app-layout {
		display: flex;
		height: 100vh;
		width: 100vw;
		background-color: var(--color-bg-base);
		color: var(--color-text-primary);
		overflow: hidden;
	}

	.app-content-wrapper {
		flex: 1;
		display: flex;
		flex-direction: column;
		height: 100%;
		overflow: hidden;
	}

	.app-topbar {
		height: 34px;
		display: flex;
		align-items: center;
		justify-content: space-between;
		background: rgba(18, 22, 28, 0.95);
		border-bottom: 1px solid rgba(255, 255, 255, 0.07);
		user-select: none;
		-webkit-app-region: drag;
	}

	.app-title-drag {
		padding-left: 1rem;
		display: flex;
		align-items: center;
		font-size: 0.75rem;
		font-weight: 600;
		letter-spacing: 0.05em;
		color: rgba(255, 255, 255, 0.5);
		height: 100%;
		flex: 1;
	}

	.app-main {
		flex: 1;
		height: calc(100% - 34px);
		position: relative;
		overflow: hidden;
	}
</style>
