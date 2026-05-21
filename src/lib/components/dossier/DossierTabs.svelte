<script lang="ts">
	import { Brain, Layers, ShieldCheck, Terminal, Database, ImageIcon, FileText } from 'lucide-svelte';

	let {
		activeDomain,
		activeTab = $bindable()
	} = $props<{
		activeDomain: 'intelligence' | 'foundation';
		activeTab: string;
	}>();
</script>

<nav class="dossier-tabs">
	{#if activeDomain === 'intelligence'}
		<button class:active={activeTab === 'synthesis'} onclick={() => (activeTab = 'synthesis')}>
			<Brain size={14} /> Synthesis
		</button>
		<button class:active={activeTab === 'forensics'} onclick={() => (activeTab = 'forensics')}>
			<ShieldCheck size={14} /> Forensic Audit
		</button>
		<button class:active={activeTab === 'thoughts'} onclick={() => (activeTab = 'thoughts')}>
			<Terminal size={14} /> Thought Stream
		</button>
		<button class:active={activeTab === 'case'} onclick={() => (activeTab = 'case')}>
			<Database size={14} /> Case Work
		</button>
	{:else}
		<button class:active={activeTab === 'artifact'} onclick={() => (activeTab = 'artifact')}>
			<ImageIcon size={14} /> Evidence Artifact
		</button>
		<button class:active={activeTab === 'raw'} onclick={() => (activeTab = 'raw')}>
			<FileText size={14} /> Raw OCR Text
		</button>
		<button class:active={activeTab === 'chunks'} onclick={() => (activeTab = 'chunks')}>
			<Layers size={14} /> Semantic Inspector
		</button>
	{/if}
</nav>

<style>
	.dossier-tabs {
		display: flex;
		padding: 0 32px;
		gap: 32px;
		border-bottom: 1px solid var(--border-subtle);
	}
	.dossier-tabs button {
		background: none;
		border: none;
		padding: 16px 0;
		font-size: 12px;
		font-weight: 700;
		color: var(--text-tertiary);
		border-bottom: 2px solid transparent;
		cursor: pointer;
		display: flex;
		align-items: center;
		gap: 10px;
	}
	.dossier-tabs button.active {
		color: var(--accent-primary);
		border-bottom-color: var(--accent-primary);
	}
</style>
