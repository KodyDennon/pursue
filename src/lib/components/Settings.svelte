<script lang="ts">
	import { onMount } from 'svelte';
	import { settingsStore } from '$lib/stores/settingsStore.svelte';
	import PersonaSettings from './settings/PersonaSettings.svelte';
	import HuggingFaceSettings from './settings/HuggingFaceSettings.svelte';
	import EnvironmentSettings from './settings/EnvironmentSettings.svelte';
	import AutomationSettings from './settings/AutomationSettings.svelte';
	import ReleaseSettings from './settings/ReleaseSettings.svelte';
	import OptimizationSettings from './settings/OptimizationSettings.svelte';

	onMount(() => {
		settingsStore.init();
	});
</script>

<div class="settings-container custom-scrollbar">
	<header class="settings-head">
		<h2>Secure System Settings</h2>
		<p>Manage your local intelligence environment and forensic archives.</p>
	</header>

	<div class="settings-grid">
		<PersonaSettings />
		<HuggingFaceSettings />
		<EnvironmentSettings />
		<AutomationSettings />
		<ReleaseSettings />
		<OptimizationSettings />
	</div>
</div>

<style>
	.settings-container {
		padding: var(--space-8xl);
		height: 100%;
		overflow-y: auto;
		max-width: 1200px;
		margin: 0 auto;
	}

	.settings-head {
		margin-bottom: var(--space-8xl);
	}

	.settings-head h2 {
		font-size: var(--text-5xl);
		margin-bottom: var(--space-md);
		color: var(--color-text-primary);
	}

	.settings-head p {
		color: var(--color-text-secondary);
		font-size: 15px;
	}

	.settings-grid {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(450px, 1fr));
		gap: var(--space-5xl);
	}
</style>
