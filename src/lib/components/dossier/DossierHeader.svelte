<script lang="ts">
	import { ChevronLeft } from 'lucide-svelte';
	import type { RecordSummary } from '$lib/types';

	let { record, onBack } = $props<{
		record: RecordSummary;
		onBack: () => void;
	}>();
</script>

<header class="dossier-header">
	<button class="back-btn" onclick={onBack}>
		<ChevronLeft size={20} /> Back to Archive
	</button>

	<div class="header-main">
		<h2>{record.title}</h2>
	</div>
</header>

<style>
	.dossier-header {
		padding: 20px 32px;
		border-bottom: 1px solid var(--border-subtle);
		background: linear-gradient(to right, rgba(231, 196, 107, 0.05), transparent);
		display: flex;
		align-items: center;
		gap: var(--space-7xl);
	}

	.back-btn {
		background: none;
		border: none;
		color: var(--text-tertiary);
		display: flex;
		align-items: center;
		gap: var(--space-md);
		font-size: var(--text-sm);
		font-weight: 800;
		text-transform: uppercase;
		cursor: pointer;
		white-space: nowrap;
	}

	.back-btn:hover {
		color: var(--text-primary);
	}

	.header-main h2 {
		font-size: var(--text-2xl);
		font-family: var(--font-display);
		font-weight: 600;
		letter-spacing: -0.01em;
		color: var(--text-primary);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}
</style>
