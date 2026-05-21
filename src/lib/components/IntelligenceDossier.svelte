<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { AlertCircle } from 'lucide-svelte';
	import MediaViewer from './MediaViewer.svelte';
	import ForensicAuditViewer from './ForensicAuditViewer.svelte';
	import SynthesisTab from './dossier/SynthesisTab.svelte';
	import CaseWorkTab from './dossier/CaseWorkTab.svelte';
	import DossierHeader from './dossier/DossierHeader.svelte';
	import DossierTabs from './dossier/DossierTabs.svelte';
	import ThoughtsTab from './dossier/ThoughtsTab.svelte';
	import ArtifactTab from './dossier/ArtifactTab.svelte';
	import RawOcrTab from './dossier/RawOcrTab.svelte';
	import ChunksTab from './dossier/ChunksTab.svelte';
	import { dossierStore } from '$lib/stores/dossierStore.svelte';
	import type { CaseSummary, RecordSummary, RecordAsset } from '$lib/types';
	import { convertFileSrc } from '@tauri-apps/api/core';
	import { openUrl } from '@tauri-apps/plugin-opener';
	import { addToast } from '$lib/toastStore';

	let {
		record,
		libraryPath = null,
		cases = [],
		selectedCaseId = null,
		onBack,
		onChanged,
		onAnalyze,
		onSynthesize
	} = $props<{
		record: RecordSummary;
		libraryPath?: string | null;
		cases: CaseSummary[];
		selectedCaseId: string | null;
		onBack: () => void;
		onChanged: () => void | Promise<void>;
		onAnalyze?: () => void;
		onSynthesize?: () => void;
	}>();

	function resolvePath(rel: string | null) {
		if (!rel || !libraryPath) return '';
		const cleanLib =
			libraryPath.endsWith('/') || libraryPath.endsWith('\\') ? libraryPath : libraryPath + '/';
		return convertFileSrc(cleanLib + rel);
	}

	let activeDomain = $state<'intelligence' | 'foundation'>('intelligence');
	let activeTab = $state<string>('synthesis');
	let viewerOpen = $state(false);

	onMount(() => {
		dossierStore.init(record);
	});

	onDestroy(() => dossierStore.destroy());

	const selectedCase = $derived(
		cases.find((item: CaseSummary) => item.id === selectedCaseId) ?? null
	);

	const images = $derived(
		(dossierStore.analysis?.assets ?? []).filter((a: RecordAsset) => a.asset_type === 'image')
	);

	const isIndexed = $derived(
		record.analysis_status === 'completed' || record.analysis_status === 'indexed'
	);

	const isSynthesisOutdated = $derived.by(() => {
		if (!record.intelligence_json || dossierStore.intelLogs.length === 0) return false;
		if (dossierStore.analysis?.ocr_text && dossierStore.intelLogs[0]) {
			const ocrTime = new Date(record.updated_at || 0).getTime();
			const intelTime = new Date(dossierStore.intelLogs[0].created_at).getTime();
			return ocrTime > intelTime + 5000;
		}
		return false;
	});

	async function openSourceProxy() {
		if (!record.document_url) return;
		try {
			await openUrl(record.document_url);
		} catch (e) {
			addToast({ type: 'error', message: `Failed to open source: ${e}` });
		}
	}

	// Ensure tab resets when domain changes if needed
	$effect(() => {
		if (activeDomain === 'foundation' && activeTab === 'synthesis') {
			activeTab = 'artifact';
		} else if (activeDomain === 'intelligence' && activeTab === 'artifact') {
			activeTab = 'synthesis';
		}
	});
</script>

<div class="intelligence-dossier glass-panel">
	<DossierHeader
		{record}
		analysis={dossierStore.analysis}
		bind:activeDomain
		{isIndexed}
		{isSynthesisOutdated}
		analysisStatus={dossierStore.analysisStatus}
		analysisProgress={dossierStore.analysisProgress}
		busy={dossierStore.busy}
		onBack={onBack}
		openSource={openSourceProxy}
		revealLocal={() => dossierStore.revealLocal()}
		runFoundationIndexing={() => dossierStore.runFoundationIndexing(onChanged, onAnalyze)}
		runDeepSynthesis={() => dossierStore.runDeepSynthesis(onChanged, onSynthesize)}
		download={() => dossierStore.download(onChanged)}
		setViewerOpen={(open) => (viewerOpen = open)}
	/>

	<DossierTabs {activeDomain} bind:activeTab />

	<div class="dossier-body">
		{#if dossierStore.error}
			<div class="error-msg">
				<AlertCircle size={18} />
				<span>System Failure: {dossierStore.error}</span>
				<button onclick={() => (dossierStore.error = null)}>Clear Error</button>
			</div>
		{/if}

		<div class="tab-content custom-scrollbar">
			{#if activeTab === 'synthesis'}
				<SynthesisTab
					{record}
					analysis={dossierStore.analysis}
					{images}
					busy={dossierStore.busy}
					onRunDeepSynthesis={() => dossierStore.runDeepSynthesis(onChanged, onSynthesize)}
				/>
			{:else if activeTab === 'forensics'}
				<ForensicAuditViewer recordId={record.id} forensics={dossierStore.forensics} {images} />
			{:else if activeTab === 'thoughts'}
				<ThoughtsTab intelLogs={dossierStore.intelLogs} />
			{:else if activeTab === 'artifact'}
				<ArtifactTab {record} {resolvePath} download={() => dossierStore.download(onChanged)} />
			{:else if activeTab === 'raw'}
				<RawOcrTab
					analysis={dossierStore.analysis}
					runFoundationIndexing={() => dossierStore.runFoundationIndexing(onChanged, onAnalyze)}
				/>
			{:else if activeTab === 'chunks'}
				<ChunksTab chunks={dossierStore.chunks} />
			{:else if activeTab === 'case'}
				<CaseWorkTab recordId={record.id} {selectedCaseId} {selectedCase} {onChanged} />
			{/if}
		</div>
	</div>
</div>

<MediaViewer {record} bind:isOpen={viewerOpen} />

<style>
	.intelligence-dossier {
		height: 100%;
		display: flex;
		flex-direction: column;
		color: #fff;
	}

	.dossier-body {
		flex: 1;
		display: flex;
		flex-direction: column;
		overflow: hidden;
	}

	.tab-content {
		flex: 1;
		overflow-y: auto;
	}

	.error-msg {
		margin: 32px;
		padding: 16px;
		background: rgba(255, 77, 77, 0.1);
		border: 1px solid rgba(255, 77, 77, 0.2);
		border-radius: 8px;
		display: flex;
		align-items: center;
		gap: 12px;
		color: #ff4d4d;
		font-size: 13px;
	}

	.error-msg button {
		margin-left: auto;
		background: rgba(255, 255, 255, 0.1);
		border: none;
		color: #fff;
		padding: 4px 12px;
		border-radius: 4px;
		cursor: pointer;
	}
</style>
