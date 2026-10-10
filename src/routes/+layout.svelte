<script lang="ts">
	import { logger } from '$lib/logger';
	logger.debug('[Layout] Script initializing...');
	import '../app.css';
	import { onMount } from 'svelte';
	import AppDock from '$lib/components/AppDock.svelte';
	import GlobalSearch from '$lib/components/GlobalSearch.svelte';
	import Toasts from '$lib/components/Toasts.svelte';
	import { settingsStore } from '$lib/stores/settingsStore.svelte';
	import { updateStore } from '$lib/stores/updateStore.svelte';

	let { children } = $props();

	onMount(() => {
		logger.debug('[Layout] Layout mounted.');
		updateStore.checkForUpdate({ automatic: true, silent: true });
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

	.app-main {
		flex: 1;
		min-height: 0;
		position: relative;
		overflow: hidden;
	}
</style>
