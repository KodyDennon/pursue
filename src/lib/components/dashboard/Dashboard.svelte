<script lang="ts">
	import { onMount } from 'svelte';
	import { logger } from '$lib/logger';
	import GridView from './GridView.svelte';
	import IntelCardsView from './IntelCardsView.svelte';
	import ListView from './ListView.svelte';
	import IntelligenceDossier from '../IntelligenceDossier.svelte';
	import type { CaseSummary, RecordSummary } from '$lib/types';

	let {
		records,
		libraryPath,
		viewMode,
		cases,
		selectedCaseId,
		selectedRecord = $bindable(null),
		onChanged,
		onAnalyze,
		onSynthesize,
		onViewMedia
	} = $props<{
		records: RecordSummary[];
		libraryPath: string | null;
		viewMode: 'grid' | 'cards' | 'list';
		cases: CaseSummary[];
		selectedCaseId: string | null;
		selectedRecord: RecordSummary | null;
		onChanged: () => void | Promise<void>;
		onAnalyze: () => void;
		onSynthesize?: () => void;
		onViewMedia: (record: RecordSummary) => void;
	}>();

	onMount(() => {
		logger.debug('[Dashboard] Dashboard mounted. View mode:', viewMode, 'Records:', records.length);
	});
</script>

<div class="dashboard-container">
	{#if selectedRecord}
		<IntelligenceDossier
			record={selectedRecord}
			{libraryPath}
			{cases}
			{selectedCaseId}
			onBack={() => (selectedRecord = null)}
			{onChanged}
			{onAnalyze}
			{onSynthesize}
		/>
	{:else if viewMode === 'grid'}
		<GridView
			{records}
			{libraryPath}
			selectedRecordId={selectedRecord?.id}
			onSelect={(r) => (selectedRecord = r)}
			onView={onViewMedia}
		/>
	{:else if viewMode === 'cards'}
		<IntelCardsView
			{records}
			{libraryPath}
			selectedRecordId={selectedRecord?.id}
			onSelect={(r) => (selectedRecord = r)}
			onView={onViewMedia}
		/>
	{:else if viewMode === 'list'}
		<ListView
			{records}
			{libraryPath}
			selectedRecordId={selectedRecord?.id}
			onSelect={(r) => (selectedRecord = r)}
			onView={onViewMedia}
		/>
	{/if}
</div>

<style>
	.dashboard-container {
		height: 100%;
		width: 100%;
		display: flex;
		flex-direction: column;
	}
</style>
