<script lang="ts">
	import { Terminal } from 'lucide-svelte';

	interface LogEntry {
		id: string;
		time: string;
		msg: string;
		type: 'info' | 'error' | 'success';
	}

	let { logs } = $props<{ logs: LogEntry[] }>();
</script>

<div class="log-side">
	<div class="section-head">
		<Terminal size={14} />
		<span>Foundation Output Log</span>
	</div>
	<div class="log-viewport custom-scrollbar">
		{#if logs.length === 0}
			<div class="empty-state">Secure ingestion logs will stream here.</div>
		{:else}
			{#each logs as log (log.id)}
				<div class="log-entry {log.type}">
					<span class="log-time">[{log.time}]</span>
					<span class="log-msg">{log.msg}</span>
				</div>
			{/each}
		{/if}
	</div>
</div>

<style>
	.log-side {
		display: flex;
		flex-direction: column;
		gap: 10px;
		overflow: hidden;
		border-left: 1px solid var(--border-subtle);
		padding-left: 28px;
		height: 100%;
	}

	.section-head {
		display: flex;
		align-items: center;
		gap: 8px;
		color: var(--text-tertiary);
		font-size: 9px;
		font-weight: 800;
		letter-spacing: 0.15em;
		text-transform: uppercase;
	}

	.log-viewport {
		flex: 1;
		background: rgba(0, 0, 0, 0.4);
		border: 1px solid var(--border-subtle);
		border-radius: var(--radius-md);
		padding: 16px;
		font-family: var(--font-mono);
		font-size: 11px;
		line-height: 1.5;
		overflow-y: auto;
		display: flex;
		flex-direction: column-reverse;
	}

	.empty-state {
		height: 100%;
		display: flex;
		align-items: center;
		justify-content: center;
		color: var(--text-tertiary);
		opacity: 0.5;
		font-style: italic;
	}

	.log-entry {
		display: flex;
		gap: 12px;
		margin-bottom: 5px;
		word-break: break-all;
	}

	.log-time {
		color: var(--text-tertiary);
		opacity: 0.6;
		flex-shrink: 0;
	}
	.log-msg {
		color: var(--text-secondary);
	}
	.log-entry.success .log-msg {
		color: var(--accent-success);
	}
	.log-entry.error .log-msg {
		color: var(--accent-danger);
	}
</style>
