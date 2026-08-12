<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { AlertCircle } from 'lucide-svelte';
	import MediaViewer from './MediaViewer.svelte';
	import ForensicAuditViewer from './ForensicAuditViewer.svelte';
	import SynthesisTab from './dossier/SynthesisTab.svelte';
	import CaseWorkTab from './dossier/CaseWorkTab.svelte';
	import DossierHeader from './dossier/DossierHeader.svelte';
	import DossierSidebar from './dossier/DossierSidebar.svelte';
	import DossierTabs from './dossier/DossierTabs.svelte';
	import ThoughtsTab from './dossier/ThoughtsTab.svelte';
	import ArtifactTab from './dossier/ArtifactTab.svelte';
	import RawOcrTab from './dossier/RawOcrTab.svelte';
	import ChunksTab from './dossier/ChunksTab.svelte';
	import { dossierStore } from '$lib/stores/dossierStore.svelte';
	import type { CaseSummary, RecordSummary, RecordAsset } from '$lib/types';
	import { openUrl } from '@tauri-apps/plugin-opener';
	import { addToast } from '$lib/stores/toastStore.svelte';
	import { resolveLibraryAssetPath } from '$lib/utils';

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
		return resolveLibraryAssetPath(libraryPath, rel);
	}

	let activeDomain = $state<'intelligence' | 'foundation'>('intelligence');
	let activeTab = $state<string>('synthesis');
	let viewerOpen = $state(false);

	onMount(() => {
		dossierStore.init(record);
	});

	onDestroy(() => dossierStore.destroy());

	const activeRecord = $derived(dossierStore.record ?? record);

	const selectedCase = $derived(
		cases.find((item: CaseSummary) => item.id === selectedCaseId) ?? null
	);

	const images = $derived(
		(dossierStore.analysis?.assets ?? []).filter((a: RecordAsset) => a.asset_type === 'image')
	);

	const isSynthesisOutdated = $derived.by(() => {
		const intelJson = activeRecord.intelligence_json || dossierStore.analysis?.intelligence_json;
		if (!intelJson || dossierStore.intelLogs.length === 0) return false;
		if (dossierStore.analysis?.ocr_text && dossierStore.intelLogs[0]) {
			const ocrTime = new Date(activeRecord.updated_at || 0).getTime();
			const intelTime = new Date(dossierStore.intelLogs[0].created_at).getTime();
			return ocrTime > intelTime + 5000;
		}
		return false;
	});

	async function openSourceProxy() {
		if (!activeRecord.document_url) return;
		try {
			await openUrl(activeRecord.document_url);
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

	// Split pane logic: show artifact on the left when viewing textual analysis
	const showArtifactSplit = $derived(
		(activeDomain === 'foundation' && activeTab !== 'artifact') ||
			(activeDomain === 'intelligence' && activeTab === 'synthesis')
	);
</script>

<div class="intelligence-dossier glass-panel">
	<DossierHeader record={activeRecord} {onBack} />

	<div class="dossier-layout">
		<main class="dossier-main-stage">
			<DossierTabs {activeDomain} bind:activeTab />

			<div class="dossier-body-container">
				{#if dossierStore.error}
					<div class="error-msg">
						<AlertCircle size={18} />
						<span>System Failure: {dossierStore.error}</span>
						<button onclick={() => (dossierStore.error = null)}>Clear Error</button>
					</div>
				{/if}

				<div class="panes-wrapper" class:split={showArtifactSplit}>
					{#if showArtifactSplit}
						<div class="artifact-pane">
							<ArtifactTab
								record={activeRecord}
								{resolvePath}
								revealLocal={() => dossierStore.revealLocal()}
								openSource={openSourceProxy}
								download={() => dossierStore.download(onChanged)}
								setViewerOpen={(open) => (viewerOpen = open)}
								compact={true}
							/>
						</div>
					{/if}

					<div class="tab-content custom-scrollbar">
						{#if activeTab === 'synthesis'}
							<SynthesisTab
								record={activeRecord}
								{images}
								busy={dossierStore.busy}
								onRunDeepSynthesis={() => dossierStore.runDeepSynthesis(onChanged, onSynthesize)}
								compact={showArtifactSplit}
							/>
						{:else if activeTab === 'forensics'}
							<ForensicAuditViewer
								recordId={activeRecord.id}
								forensics={dossierStore.forensics}
								{images}
							/>
						{:else if activeTab === 'thoughts'}
							<ThoughtsTab intelLogs={dossierStore.intelLogs} />
						{:else if activeTab === 'artifact'}
							<ArtifactTab
								record={activeRecord}
								{resolvePath}
								revealLocal={() => dossierStore.revealLocal()}
								openSource={openSourceProxy}
								download={() => dossierStore.download(onChanged)}
								setViewerOpen={(open) => (viewerOpen = open)}
							/>
						{:else if activeTab === 'raw'}
							<RawOcrTab
								analysis={dossierStore.analysis}
								runFoundationIndexing={() =>
									dossierStore.runFoundationIndexing(onChanged, onAnalyze)}
							/>
						{:else if activeTab === 'chunks'}
							<ChunksTab chunks={dossierStore.chunks} />
						{:else if activeTab === 'case'}
							<CaseWorkTab recordId={activeRecord.id} {selectedCaseId} {selectedCase} {onChanged} />
						{/if}
					</div>
				</div>
			</div>
		</main>

		<DossierSidebar
			record={activeRecord}
			analysis={dossierStore.analysis}
			bind:activeDomain
			{isSynthesisOutdated}
			analysisStatus={dossierStore.analysisStatus}
			analysisProgress={dossierStore.analysisProgress}
			runDeepSynthesis={() => dossierStore.runDeepSynthesis(onChanged, onSynthesize)}
		/>
	</div>
</div>

<MediaViewer record={activeRecord} bind:isOpen={viewerOpen} />

<style>
	.intelligence-dossier {
		height: 100%;
		display: flex;
		flex-direction: column;
		color: #fff;
		overflow: hidden;
	}

	.dossier-layout {
		flex: 1;
		display: flex;
		overflow: hidden;
	}

	.dossier-main-stage {
		flex: 1;
		display: flex;
		flex-direction: column;
		overflow: hidden;
	}

	.dossier-body-container {
		flex: 1;
		display: flex;
		flex-direction: column;
		overflow: hidden;
	}

	.panes-wrapper {
		flex: 1;
		display: flex;
		overflow: hidden;
	}

	.panes-wrapper.split {
		gap: 1px;
		background: var(--color-border-subtle);
	}

	.artifact-pane {
		width: 45%;
		background: #000;
		overflow: hidden;
		border-right: 1px solid var(--color-border-subtle);
	}

	.tab-content {
		flex: 1;
		overflow-y: auto;
		background: rgba(8, 9, 12, 0.2);
	}

	.error-msg {
		margin: var(--space-5xl);
		padding: 12px 16px;
		background: rgba(255, 77, 77, 0.1);
		border: 1px solid rgba(255, 77, 77, 0.2);
		border-radius: var(--radius-base);
		display: flex;
		align-items: center;
		gap: var(--space-xl);
		color: var(--color-accent-danger);
		font-size: var(--text-md);
	}

	.error-msg button {
		margin-left: auto;
		background: rgba(255, 255, 255, 0.1);
		border: none;
		color: #fff;
		padding: 4px 12px;
		border-radius: var(--radius-xs);
		cursor: pointer;
	}
</style>
