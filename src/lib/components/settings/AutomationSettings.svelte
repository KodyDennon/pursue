<script lang="ts">
	import { ShieldCheck } from 'lucide-svelte';
	import { settingsStore } from '$lib/stores/settingsStore.svelte';
</script>

<section class="settings-section glass-panel">
	<div class="s-header">
		<ShieldCheck size={18} class="accent-icon" />
		<h3>Automation & Pipeline</h3>
	</div>
	<div class="s-body">
		<div class="toggle-item">
			<div class="t-info">
				<strong>Auto-Retrieval Pipeline</strong>
				<span>Automatically download official sources when synced.</span>
			</div>
			<button
				class="toggle"
				aria-label="Toggle"
				class:active={settingsStore.agentSettings.auto_sync}
				onclick={() => {
					settingsStore.agentSettings.auto_sync = !settingsStore.agentSettings.auto_sync;
					settingsStore.saveAgentSettings();
				}}
			></button>
		</div>
		<div class="toggle-item">
			<div class="t-info">
				<strong>Neural Post-Processing</strong>
				<span>Automatically initiate Gemma 4 extraction after download.</span>
			</div>
			<button
				class="toggle"
				aria-label="Toggle"
				class:active={settingsStore.agentSettings.auto_analyze}
				onclick={() => {
					settingsStore.agentSettings.auto_analyze = !settingsStore.agentSettings.auto_analyze;
					settingsStore.saveAgentSettings();
				}}
			></button>
		</div>
	</div>
</section>

<style>
	.settings-section {
		background: var(--bg-surface);
		border: 1px solid var(--border-subtle);
		border-radius: var(--radius-lg);
		display: flex;
		flex-direction: column;
	}

	.s-header {
		padding: 24px;
		display: flex;
		align-items: center;
		gap: 16px;
		border-bottom: 1px solid var(--border-subtle);
	}

	.s-header h3 {
		font-size: 16px;
		font-weight: 600;
		margin: 0;
	}

	.s-body {
		padding: 24px;
		flex: 1;
		display: flex;
		flex-direction: column;
		gap: 24px;
	}

	.toggle-item {
		display: flex;
		justify-content: space-between;
		align-items: center;
		gap: 24px;
	}

	.t-info {
		display: flex;
		flex-direction: column;
		gap: 4px;
	}

	.t-info strong {
		font-size: 14px;
		color: var(--text-primary);
	}

	.t-info span {
		font-size: 12px;
		color: var(--text-secondary);
	}

	.toggle {
		width: 36px;
		height: 20px;
		background: #2a2d35;
		border-radius: 10px;
		position: relative;
		cursor: pointer;
		border: none;
		padding: 0;
		transition: background 0.2s;
	}

	.toggle.active {
		background: var(--accent-primary);
	}

	.toggle::after {
		content: '';
		position: absolute;
		width: 14px;
		height: 14px;
		background: #fff;
		border-radius: 50%;
		top: 3px;
		left: 3px;
		transition: transform 0.2s;
	}

	.toggle.active::after {
		transform: translateX(16px);
	}

	:global(.accent-icon) {
		color: var(--accent-primary);
	}
</style>
