<script lang="ts">
	import { logger } from '$lib/logger';
	logger.debug('[Layout] Script initializing...');
	import '../app.css';
	import { onMount } from 'svelte';
	import AppDock from '$lib/components/AppDock.svelte';
	import GlobalSearch from '$lib/components/GlobalSearch.svelte';
	import Toasts from '$lib/components/Toasts.svelte';

	let { children } = $props();

	onMount(() => {
		logger.debug('[Layout] Layout mounted.');
	});
</script>

<div class="app-layout">
	<AppDock />
	<main class="app-main">
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
		background-color: var(--bg-base);
		color: var(--text-primary);
		overflow: hidden;
	}

	.app-main {
		flex: 1;
		height: 100%;
		position: relative;
		overflow: hidden; /* Each view manages its own scrolling */
	}
</style>
