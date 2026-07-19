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
		const timer = window.setTimeout(() => {
			void updateStore.checkForUpdate({ silent: true, automatic: true });
		}, 10_000);
		return () => window.clearTimeout(timer);
	});

	$effect(() => {
		if (typeof document !== 'undefined') {
			document.documentElement.classList.toggle('performance-mode', settingsStore.performanceMode);
		}
	});
</script>

<div class="app-layout" data-tauri-drag-region>
	<AppDock />
	<main class="app-main" data-tauri-drag-region={false}>
		{@render children()}
	</main>
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

	.app-main {
		flex: 1;
		height: 100%;
		position: relative;
		overflow: hidden; /* Each view manages its own scrolling */
	}
</style>
