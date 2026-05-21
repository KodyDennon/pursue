<script lang="ts">
	import { onMount } from 'svelte';
	import { convertFileSrc, invoke } from '@tauri-apps/api/core';
	import { openPath, openUrl } from '@tauri-apps/plugin-opener';
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
	import { addToast } from '$lib/toastStore';
	import { listen, type UnlistenFn } from '@tauri-apps/api/event';
	import type {
		AnalysisReport,
		CaseSummary,
		RecordSummary,
		RecordAsset,
		RecordForensics,
		IntelligenceLog,
		AnalysisChunk
	} from '$lib/types';

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
	let analysis = $state<AnalysisReport | null>(null);
	let forensics = $state<RecordForensics[]>([]);
	let intelLogs = $state<IntelligenceLog[]>([]);
	let chunks = $state<AnalysisChunk[]>([]);
	let busy = $state<string | null>(null);
	let error = $state<string | null>(null);
	let viewerOpen = $state(false);

	// Real-time analysis status
	let analysisStatus = $state<string | null>(null);
	let analysisProgress = $state(0);

	const selectedCase = $derived(
		cases.find((item: CaseSummary) => item.id === selectedCaseId) ?? null
	);

	const images = $derived(
		(analysis?.assets ?? []).filter((a: RecordAsset) => a.asset_type === 'image')
	);

	const isIndexed = $derived(record.analysis_status === 'completed' || record.analysis_status === 'indexed');

	const isSynthesisOutdated = $derived.by(() => {
		if (!record.intelligence_json || intelLogs.length === 0) return false;
		// Simple heuristic: if processed_at (OCR) is newer than the latest synthesis log
		if (analysis?.ocr_text && intelLogs[0]) {
			const ocrTime = new Date(record.updated_at || 0).getTime();
			const intelTime = new Date(intelLogs[0].created_at).getTime();
			return ocrTime > intelTime + 5000; // 5s buffer
		}
		return false;
	});

	async function loadAnalysis() {
		if (!record) return;
		error = null;
		try {
			analysis = await invoke<AnalysisReport | null>('get_analysis_result', { id: record.id });

			if (record.analysis_status === 'completed' || record.analysis_status === 'indexed') {
				await Promise.all([loadForensics(), loadChunks()]);
			}
		} catch (e) {
			error = String(e);
		}
	}

	async function loadForensics() {
		try {
			forensics = await invoke<RecordForensics[]>('get_forensic_report', { id: record.id });
			intelLogs = await invoke<IntelligenceLog[]>('get_intelligence_logs', { id: record.id });
		} catch (e) {
			console.error('Forensic load failed:', e);
		}
	}

	async function loadChunks() {
		try {
			chunks = await invoke<AnalysisChunk[]>('get_record_chunks', { id: record.id });
		} catch (e) {
			console.error('Chunk load failed:', e);
		}
	}

	async function download() {
		busy = 'download';
		error = null;
		try {
			if (!record.document_url) throw new Error('No source URL available');

			// Use the hardened backend proxy to bypass CORS and browser header restrictions
			const bytes = await invoke<number[]>('proxy_fetch_url', {
				url: record.document_url
			});

			await invoke('download_record_with_bytes', {
				id: record.id,
				url: record.document_url,
				bytes: bytes
			});

			if (onChanged) await onChanged();
			await loadAnalysis();
			addToast({ type: 'success', message: 'Evidence retrieved and vaulted.', duration: 3000 });
		} catch (e) {
			error = String(e);
			addToast({ type: 'error', message: `Download failed: ${e}` });
		} finally {
			busy = null;
		}
	}

	async function runFoundationIndexing() {
		busy = 'indexing';
		error = null;
		try {
			if (onAnalyze) onAnalyze();
			await invoke('index_record', { id: record.id, current: 1, total: 1 });
			if (onChanged) await onChanged();
			await loadAnalysis();
			addToast({ type: 'success', message: 'Foundation Indexed Successfully', duration: 2000 });
		} catch (e) {
			error = String(e);
		} finally {
			busy = null;
		}
	}

	async function runDeepSynthesis() {
		busy = 'synthesis';
		error = null;
		try {
			if (onSynthesize) onSynthesize();
			const report = await invoke<AnalysisReport>('synthesize_intelligence', { id: record.id });
			analysis = report;
			if (onChanged) await onChanged();
			await loadForensics();
			addToast({ type: 'success', message: 'Intelligence Synthesis Complete', duration: 3000 });
		} catch (e) {
			error = String(e);
		} finally {
			busy = null;
		}
	}

	async function openSource() {
		if (!record.document_url) return;
		try {
			await openUrl(record.document_url);
		} catch (e) {
			addToast({ type: 'error', message: `Failed to open source: ${e}` });
		}
	}

	async function revealLocal() {
		if (!record.local_path) return;
		busy = 'open-path';
		try {
			const path = await invoke<string>('get_record_artifact_path', { id: record.id });
			await openPath(path);
		} catch (e) {
			error = String(e);
		} finally {
			busy = null;
		}
	}

	onMount(() => {
		loadAnalysis();

		let unlisten: UnlistenFn;
		listen<{
			record_id: string;
			status: string;
			token_index?: number;
			token_text?: string;
		}>('analysis-progress', (event) => {
			const payload = event.payload;
			if (payload.record_id === record.id) {
				if (payload.status === 'synthesizing' || payload.status === 'synthesizing-start') {
					analysisStatus = 'Neural Synthesis In Progress...';
					if (payload.token_index) {
						analysisProgress = Math.round((payload.token_index / 2048) * 100);
					}
				} else if (payload.status === 'loading-model') {
					analysisStatus = 'Waking Neural Engine...';
				} else if (payload.status === 'extracting-foundation') {
					analysisStatus = 'Foundation OCR In Progress...';
					analysisProgress = 20;
				}
			}
		}).then((u) => (unlisten = u));

		return () => {
			if (unlisten) unlisten();
		};
	});

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
		{analysis}
		bind:activeDomain
		{isIndexed}
		{isSynthesisOutdated}
		{analysisStatus}
		{analysisProgress}
		{busy}
		onBack={onBack}
		openSource={openSource}
		revealLocal={revealLocal}
		runFoundationIndexing={runFoundationIndexing}
		runDeepSynthesis={runDeepSynthesis}
		download={download}
		setViewerOpen={(open) => (viewerOpen = open)}
	/>

	<DossierTabs {activeDomain} bind:activeTab />

	<div class="dossier-body">
		{#if error}
			<div class="error-msg">
				<AlertCircle size={18} />
				<span>System Failure: {error}</span>
				<button onclick={() => (error = null)}>Clear Error</button>
			</div>
		{/if}

		<div class="tab-content custom-scrollbar">
			{#if activeTab === 'synthesis'}
				<SynthesisTab {record} {analysis} {images} {busy} onRunDeepSynthesis={runDeepSynthesis} />
			{:else if activeTab === 'forensics'}
				<ForensicAuditViewer recordId={record.id} {forensics} {images} />
			{:else if activeTab === 'thoughts'}
				<ThoughtsTab {intelLogs} />
			{:else if activeTab === 'artifact'}
				<ArtifactTab {record} {resolvePath} {download} />
			{:else if activeTab === 'raw'}
				<RawOcrTab {analysis} {runFoundationIndexing} />
			{:else if activeTab === 'chunks'}
				<ChunksTab {chunks} />
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
