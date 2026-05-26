<script lang="ts">
	import { onMount } from 'svelte';
	import { appStore } from '$lib/stores/appStore.svelte';

	let takingLong = $state(false);

	onMount(() => {
		const timer = setTimeout(() => {
			takingLong = true;
		}, 10000);
		return () => clearTimeout(timer);
	});
</script>

<div class="system-splash">
	<div class="splash-content">
		<div class="loader-spinner"></div>
		<h2>INTELLIGENCE OS INITIALIZING</h2>
		<p>Syncing local evidence vault and neural models...</p>
		<div class="boot-log">
			{#if appStore.bootLogs.length > 0}
				{#each appStore.bootLogs as log (log)}
					<span>{log}</span>
				{/each}
			{:else}
				<span>[SYSTEM] Mounting secure database...</span>
				<span>[SYSTEM] Initializing vector search engine...</span>
				<span>[SYSTEM] Loading AARO official source records...</span>
			{/if}
		</div>

		{#if takingLong}
			<div class="long-load-msg">
				<span>System is performing high-integrity data verification.</span>
				<span>This may take a moment on large archives...</span>
			</div>
		{/if}
	</div>
</div>

<style>
	.system-splash {
		position: fixed;
		top: 0;
		left: 0;
		right: 0;
		bottom: 0;
		background: #000;
		z-index: 1000;
		display: flex;
		align-items: center;
		justify-content: center;
	}

	.splash-content {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 24px;
		text-align: center;
	}

	h2 {
		font-size: 24px;
		color: var(--text-primary);
		margin: 0;
	}

	p {
		color: var(--text-secondary);
		margin: 0;
	}

	.loader-spinner {
		width: 40px;
		height: 40px;
		border: 3px solid rgba(231, 196, 107, 0.1);
		border-top: 3px solid var(--accent-primary);
		border-radius: 50%;
		animation: spin 1s linear infinite;
		margin-bottom: 24px;
	}

	@keyframes spin {
		0% {
			transform: rotate(0deg);
		}
		100% {
			transform: rotate(360deg);
		}
	}

	.boot-log {
		margin-top: 24px;
		display: flex;
		flex-direction: column;
		gap: 8px;
		font-family: var(--font-mono);
		font-size: 11px;
		color: var(--accent-primary);
		opacity: 0.7;
		text-align: left;
		width: 300px;
	}

	.long-load-msg {
		margin-top: 32px;
		display: flex;
		flex-direction: column;
		gap: 4px;
		font-size: 10px;
		color: var(--text-tertiary);
		text-transform: uppercase;
		letter-spacing: 0.05em;
		animation: pulse 2s ease-in-out infinite;
	}

	@keyframes pulse {
		0%,
		100% {
			opacity: 0.4;
		}
		50% {
			opacity: 1;
		}
	}
</style>
