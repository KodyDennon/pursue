<script lang="ts">
	import '../app.css';
	import { onMount } from 'svelte';
	import { activeTheme } from '$lib/theme';
	import AppDock from '$lib/components/AppDock.svelte';
	import GlobalSearch from '$lib/components/GlobalSearch.svelte';
	import Toasts from '$lib/components/Toasts.svelte';
	import { checkForUpdates } from '$lib/updater';

	let { children } = $props();

	onMount(() => {
		// Re-apply theme on client load just to be sure
		activeTheme.subscribe(() => {
			// Handled by the store itself, but subscribe ensures it binds
		});

		// Check for system updates silently on startup
		setTimeout(() => {
			checkForUpdates(true);
		}, 3000); // Wait 3s to let system settle
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
