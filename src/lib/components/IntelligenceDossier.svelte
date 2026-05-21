<script lang="ts">
	import { onMount } from 'svelte';
	import { convertFileSrc, invoke } from '@tauri-apps/api/core';
	import { openPath, openUrl } from '@tauri-apps/plugin-opener';
	import {
		AlertCircle,
		Brain,
		FileText,
		ImageIcon,
		ChevronLeft,
		Download,
		ExternalLink,
		HardDrive,
		ShieldCheck,
		Activity,
		Terminal,
		Maximize2,
		Fingerprint,
		Database,
		Layers,
		Search,
		Clock
	} from 'lucide-svelte';
	import MediaViewer from './MediaViewer.svelte';
	import ForensicAuditViewer from './ForensicAuditViewer.svelte';
	import { addToast } from '$lib/toastStore';
	import { listen, type UnlistenFn } from '@tauri-apps/api/event';
	import type {
		AnalysisReport,
		CaseSummary,
		DownloadResult,
		ExportResult,
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
	let noteBody = $state('');
	let exportPath = $state<string | null>(null);
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

	async function runFoundationIndexing(forceOcr = false) {
		busy = 'indexing';
		error = null;
		try {
			if (onAnalyze) onAnalyze();
			await invoke('index_record', { id: record.id, forceOcr, current: 1, total: 1 });
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

	async function addToCase() {
		if (!selectedCaseId) return;
		busy = 'case-add';
		error = null;
		try {
			await invoke('add_record_to_case', {
				request: { case_id: selectedCaseId, record_id: record.id, notes: noteBody.trim() || null }
			});
			if (onChanged) await onChanged();
		} catch (e) {
			error = String(e);
		} finally {
			busy = null;
		}
	}

	async function addNote() {
		if (!selectedCaseId || !noteBody.trim()) return;
		busy = 'case-note';
		error = null;
		try {
			await invoke('update_case_notes', {
				request: { case_id: selectedCaseId, record_id: record.id, body: noteBody.trim() }
			});
			noteBody = '';
			if (onChanged) await onChanged();
		} catch (e) {
			error = String(e);
		} finally {
			busy = null;
		}
	}

	async function exportCase(format: 'markdown' | 'html') {
		if (!selectedCaseId) return;
		busy = `export-${format}`;
		error = null;
		try {
			const result = await invoke<ExportResult>('export_case', {
				request: { case_id: selectedCaseId, format }
			});
			exportPath = result.absolute_path;
			if (onChanged) await onChanged();
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
	<header class="dossier-header">
		<div class="h-nav">
			<button class="back-btn" onclick={onBack}>
				<ChevronLeft size={20} /> Back to Archive
			</button>

			<div class="domain-selector">
				<button class:active={activeDomain === 'intelligence'} onclick={() => activeDomain = 'intelligence'}>
					<Brain size={14} /> INTELLIGENCE
				</button>
				<button class:active={activeDomain === 'foundation'} onclick={() => activeDomain = 'foundation'}>
					<Layers size={14} /> FOUNDATION
				</button>
			</div>
		</div>

		<div class="header-main">
			<div class="h-top">
				<span class="source-tag">{record.source_type.toUpperCase()} SOURCE</span>
				<span class="engine-tag">{analysis?.engine?.toUpperCase() || 'CORE_SYSTEM'}</span>
				{#if record.local_path}
					<span class="status-tag success">VERIFIED LOCAL</span>
				{:else}
					<span class="status-tag cloud">REMOTE TARGET</span>
				{/if}
			</div>
			<h2>{record.title}</h2>
			<div class="header-meta">
				<div class="m-item"><Fingerprint size={12} /> {record.id.substring(0, 12)}</div>
				<div class="m-item"><Database size={12} /> {record.agency || 'UNKNOWN'}</div>
				<div class="m-item"><Clock size={12} /> {record.release_date || 'UNDATED'}</div>
				<div class="m-item status" class:completed={record.analysis_status === 'completed'}>
					{record.analysis_status?.toUpperCase() || 'PENDING'}
				</div>
			</div>
		</div>

		<div class="header-actions">
			{#if record.document_url}
				<button class="btn-premium" onclick={openSource}>
					<ExternalLink size={14} /> Source
				</button>
			{/if}
			{#if record.local_path}
				<button class="btn-premium" onclick={revealLocal} disabled={busy === 'open-path'}>
					<HardDrive size={14} /> Local Artifact
				</button>
				
				{#if record.analysis_status !== 'completed' && record.analysis_status !== 'indexed'}
					<button class="btn-premium accent" onclick={() => runFoundationIndexing(false)} disabled={!!busy}>
						<Search size={14} /> Index Foundation
					</button>
				{:else}
					<button class="btn-premium" onclick={() => runFoundationIndexing(true)} disabled={!!busy}>
						<Layers size={14} /> Re-Audit Foundation
					</button>
					<button class="btn-premium accent" onclick={runDeepSynthesis} disabled={!!busy}>
						<Brain size={14} /> Neural Synthesis
					</button>
				{/if}
				
				<button class="btn-premium" onclick={() => (viewerOpen = true)}>
					<Maximize2 size={14} /> View Media
				</button>
			{:else}
				<button class="btn-premium accent" onclick={download} disabled={!!busy}>
					<Download size={14} /> Download Evidence
				</button>
			{/if}
		</div>

		{#if isSynthesisOutdated}
			<div class="alert-banner warning">
				<AlertCircle size={14} />
				<span>FOUNDATION UPDATED: New OCR data available. Re-run Neural Synthesis to align intelligence.</span>
				<button onclick={runDeepSynthesis}>SYNTHESIZE NOW</button>
			</div>
		{/if}

		{#if analysisStatus}
			<div class="analysis-hud">
				<div class="hud-bar">
					<div class="hud-fill" style="width: {analysisProgress}%"></div>
				</div>
				<div class="hud-label">
					<Activity size={10} class="pulse" />
					<span>{analysisStatus.toUpperCase()}</span>
					<span class="pct">{analysisProgress}%</span>
				</div>
			</div>
		{/if}
	</header>

	<nav class="dossier-tabs">
		{#if activeDomain === 'intelligence'}
			<button class:active={activeTab === 'synthesis'} onclick={() => activeTab = 'synthesis'}>
				<Brain size={14} /> Synthesis
			</button>
			<button class:active={activeTab === 'forensics'} onclick={() => activeTab = 'forensics'}>
				<ShieldCheck size={14} /> Forensic Audit
			</button>
			<button class:active={activeTab === 'thoughts'} onclick={() => activeTab = 'thoughts'}>
				<Terminal size={14} /> Thought Stream
			</button>
			<button class:active={activeTab === 'case'} onclick={() => activeTab = 'case'}>
				<Database size={14} /> Case Work
			</button>
		{:else}
			<button class:active={activeTab === 'artifact'} onclick={() => activeTab = 'artifact'}>
				<ImageIcon size={14} /> Evidence Artifact
			</button>
			<button class:active={activeTab === 'raw'} onclick={() => activeTab = 'raw'}>
				<FileText size={14} /> Raw OCR Text
			</button>
			<button class:active={activeTab === 'chunks'} onclick={() => activeTab = 'chunks'}>
				<Layers size={14} /> Semantic Inspector
			</button>
		{/if}
	</nav>

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
				<div class="view-padding">
					{#if record.intelligence_json}
						{@const intel = JSON.parse(record.intelligence_json)}
						<div class="intel-grid">
							<div class="intel-main">
								<section class="intel-card-section">
									<header class="section-head"><span class="prefix">EXECUTIVE SUMMARY</span></header>
									<p class="para">{intel.object_description || 'No summary available.'}</p>
								</section>

								<div class="data-grid-tactical">
									<div class="t-card">
										<span class="t-label">TARGET DATE</span>
										<span class="t-val">{intel.incident_date || record.incident_date || 'UNDISCLOSED'}</span>
									</div>
									<div class="t-card">
										<span class="t-label">GEOSPATIAL TAG</span>
										<span class="t-val">{intel.location || record.incident_location || 'GLOBAL'}</span>
									</div>
									<div class="t-card full">
										<span class="t-label">AGENCY ASSOCIATIONS</span>
										<div class="t-tags">
											{#each intel.agencies || [] as agency (agency)}
												<span class="f-tag">{agency}</span>
											{/each}
										</div>
									</div>
								</div>
								
								<section class="intel-card-section">
									<header class="section-head"><span class="prefix">QUALITATIVE OBSERVATIONS</span></header>
									<p class="para">{intel.pilot_observations || 'No observational data resolved.'}</p>
								</section>
							</div>

							<aside class="intel-sidebar">
								<div class="fidelity-dial-wrap">
									<span class="t-label">SYNTHESIS FIDELITY</span>
									<div class="dial">
										{Math.round((intel.intelligence_score || 0.6) * 100)}%
									</div>
								</div>
								{#if images.length > 0}
									<div class="mini-gallery">
										<span class="t-label">VISUAL EVIDENCE</span>
										<div class="g-grid">
											{#each images.slice(0, 4) as img (img.id)}
												<img src={convertFileSrc(img.local_path)} alt="Evidence" />
											{/each}
										</div>
									</div>
								{/if}
							</aside>
						</div>
					{:else}
						<div class="pending-state">
							<Brain size={48} class="accent-icon" />
							<h3>Deep Intelligence Synthesis Pending</h3>
							<p>Gemma 4 must perform a semantic audit to generate executive intelligence.</p>
							<button class="primary-btn" onclick={runDeepSynthesis} disabled={busy === 'synthesis'}>
								RUN NEURAL SYNTHESIS
							</button>
						</div>
					{/if}
				</div>
			{:else if activeTab === 'forensics'}
				<ForensicAuditViewer recordId={record.id} {forensics} {images} />
			{:else if activeTab === 'thoughts'}
				<div class="view-padding">
					<header class="section-head"><span class="prefix">NEURAL LOGSTACK</span></header>
					<div class="log-stack">
						{#each intelLogs as log (log.id)}
							<div class="log-entry-item">
								<header>{log.model_id} @ {new Date(log.created_at).toLocaleTimeString()}</header>
								<pre>{log.response_json}</pre>
							</div>
						{/each}
					</div>
				</div>
			{:else if activeTab === 'artifact'}
				<div class="view-padding">
					{#if record.local_path}
						<div class="artifact-preview">
							<iframe src={resolvePath(record.local_path)} title="Evidence Document"></iframe>
						</div>
					{:else}
						<div class="pending-state">
							<Download size={48} />
							<h3>Local Artifact Missing</h3>
							<button onclick={download}>Download Source</button>
						</div>
					{/if}
				</div>
			{:else if activeTab === 'raw'}
				<div class="view-padding">
					{#if analysis?.ocr_text}
						<header class="section-head"><span class="prefix">FOUNDATION OCR LOG</span></header>
						<pre class="raw-text-block">{analysis.ocr_text}</pre>
					{:else}
						<div class="pending-state">
							<FileText size={48} />
							<h3>No Foundation Index</h3>
							<button onclick={() => runFoundationIndexing()}>Initialize Foundation</button>
						</div>
					{/if}
				</div>
			{:else if activeTab === 'chunks'}
				<div class="view-padding">
					<header class="section-head"><span class="prefix">SEMANTIC CHUNK MANIFEST</span></header>
					<div class="chunk-list">
						{#each chunks as chunk (chunk.id)}
							<div class="chunk-card">
								<span class="c-idx">CHUNK_{chunk.chunk_index.toString().padStart(3, '0')}</span>
								<p>{chunk.text}</p>
							</div>
						{/each}
					</div>
				</div>
			{:else if activeTab === 'case'}
				<div class="view-padding">
					<header class="section-head"><span class="prefix">TACTICAL CASE INTEGRATION</span></header>
					<section class="case-work-section">
						<p class="case-status">
							{selectedCase
								? `Target Case: ${selectedCase.title}`
								: 'No primary case active. Select a case from the Tactical Dashboard.'}
						</p>
						<textarea bind:value={noteBody} rows="5" placeholder="Append forensic observations to case log..."
						></textarea>
						<div class="case-actions">
							<button class="btn-premium" onclick={addToCase} disabled={!selectedCaseId || busy === 'case-add'}
								>Add to Case</button
							>
							<button
								class="btn-premium"
								onclick={addNote}
								disabled={!selectedCaseId || !noteBody.trim() || busy === 'case-note'}>Post Note</button
							>
							<button
								class="btn-premium"
								onclick={() => exportCase('markdown')}
								disabled={!selectedCaseId || busy === 'export-markdown'}>Export MD</button
							>
							<button
								class="btn-premium"
								onclick={() => exportCase('html')}
								disabled={!selectedCaseId || busy === 'export-html'}>Export HTML</button
							>
						</div>
						{#if exportPath}
							<p class="path-line">Dossier exported to: {exportPath}</p>
						{/if}
					</section>
				</div>
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

	.dossier-header {
		padding: 24px 32px;
		border-bottom: 1px solid var(--border-subtle);
		background: linear-gradient(to bottom, rgba(231, 196, 107, 0.05), transparent);
	}

	.h-nav {
		display: flex;
		justify-content: space-between;
		align-items: center;
		margin-bottom: 24px;
	}

	.back-btn {
		background: none;
		border: none;
		color: var(--text-tertiary);
		display: flex;
		align-items: center;
		gap: 8px;
		font-size: 11px;
		font-weight: 800;
		text-transform: uppercase;
		cursor: pointer;
	}

	.domain-selector {
		display: flex;
		background: #000;
		padding: 4px;
		border-radius: 6px;
		border: 1px solid var(--border-subtle);
		gap: 4px;
	}

	.domain-selector button {
		padding: 6px 12px;
		font-size: 10px;
		font-weight: 800;
		border-radius: 4px;
		color: var(--text-tertiary);
		display: flex;
		align-items: center;
		gap: 8px;
		border: none;
		background: none;
		cursor: pointer;
		transition: all 0.2s;
	}

	.domain-selector button.active {
		background: rgba(255, 255, 255, 0.05);
		color: var(--accent-primary);
	}

	.header-main h2 {
		font-size: 22px;
		margin: 8px 0;
		letter-spacing: -0.01em;
	}

	.h-top {
		display: flex;
		gap: 12px;
		align-items: center;
	}
	.source-tag, .engine-tag {
		font-size: 9px;
		font-weight: 900;
		padding: 2px 6px;
		border-radius: 3px;
		background: rgba(255, 255, 255, 0.05);
		color: var(--text-tertiary);
	}
	.status-tag {
		font-size: 9px;
		font-weight: 900;
	}
	.status-tag.success { color: var(--accent-success); }
	.status-tag.cloud { color: #3296ff; }

	.header-meta {
		display: flex;
		gap: 20px;
		font-size: 11px;
		color: var(--text-tertiary);
		margin-top: 4px;
	}
	.m-item {
		display: flex;
		align-items: center;
		gap: 6px;
	}
	.status.completed {
		color: var(--accent-success);
		font-weight: 900;
	}

	.header-actions {
		display: flex;
		gap: 12px;
		margin-top: 24px;
	}


	.alert-banner {
		margin-top: 20px;
		padding: 10px 16px;
		border-radius: 6px;
		display: flex;
		align-items: center;
		gap: 12px;
		font-size: 11px;
		font-weight: 700;
	}
	.alert-banner.warning {
		background: rgba(231, 196, 107, 0.1);
		border: 1px solid rgba(231, 196, 107, 0.2);
		color: var(--accent-primary);
	}
	.alert-banner button {
		margin-left: auto;
		background: var(--accent-primary);
		color: #000;
		border: none;
		padding: 4px 10px;
		border-radius: 4px;
		font-size: 10px;
		font-weight: 900;
		cursor: pointer;
	}

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

	.tab-content {
		flex: 1;
		overflow-y: auto;
	}
	.view-padding {
		padding: 32px;
	}

	.section-head {
		margin-bottom: 20px;
		border-bottom: 1px solid rgba(255, 255, 255, 0.05);
		padding-bottom: 8px;
	}
	.prefix {
		font-size: 9px;
		font-weight: 900;
		letter-spacing: 0.15em;
		color: var(--text-tertiary);
	}

	.para {
		font-size: 14px;
		line-height: 1.7;
		color: var(--text-primary);
	}

	.data-grid-tactical {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: 16px;
		margin: 32px 0;
	}
	.t-card {
		background: rgba(255, 255, 255, 0.02);
		border: 1px solid var(--border-subtle);
		padding: 16px;
		border-radius: 8px;
		display: flex;
		flex-direction: column;
		gap: 4px;
	}
	.t-card.full { grid-column: span 2; }
	.t-label {
		font-size: 9px;
		font-weight: 900;
		color: var(--text-tertiary);
	}
	.t-val {
		font-size: 14px;
		font-weight: 600;
	}
	.t-tags {
		display: flex;
		flex-wrap: wrap;
		gap: 6px;
		margin-top: 8px;
	}

	.fidelity-dial-wrap {
		background: #000;
		border: 1px solid var(--border-subtle);
		padding: 20px;
		border-radius: 12px;
		text-align: center;
	}
	.dial {
		font-size: 32px;
		font-weight: 800;
		margin-top: 12px;
		color: var(--accent-primary);
	}

	.case-work-section {
		display: flex;
		flex-direction: column;
		gap: 20px;
	}
	.case-status {
		font-size: 13px;
		font-weight: 600;
		color: var(--accent-primary);
	}
	.case-work-section textarea {
		background: rgba(0, 0, 0, 0.3);
		border: 1px solid var(--border-subtle);
		border-radius: 8px;
		padding: 16px;
		color: #fff;
		font-family: var(--font-display);
		resize: none;
	}
	.case-actions {
		display: flex;
		gap: 12px;
	}
	.path-line {
		font-size: 11px;
		color: var(--text-tertiary);
		font-family: var(--font-mono);
	}

	.artifact-preview {
		height: 600px;
		background: #000;
		border-radius: 12px;
		overflow: hidden;
		border: 1px solid var(--border-subtle);
	}
	.artifact-preview iframe {
		width: 100%;
		height: 100%;
		border: none;
	}

	.raw-text-block {
		background: #000;
		padding: 24px;
		border-radius: 12px;
		font-family: var(--font-mono);
		font-size: 12px;
		line-height: 1.8;
		white-space: pre-wrap;
		color: var(--text-secondary);
		border: 1px solid var(--border-subtle);
	}

	.chunk-list {
		display: flex;
		flex-direction: column;
		gap: 16px;
	}
	.chunk-card {
		background: rgba(255, 255, 255, 0.02);
		border: 1px solid var(--border-subtle);
		padding: 16px;
		border-radius: 8px;
	}
	.c-idx {
		font-family: var(--font-mono);
		font-size: 10px;
		color: var(--accent-primary);
		margin-bottom: 8px;
		display: block;
	}

	.pending-state {
		height: 400px;
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		text-align: center;
		gap: 16px;
		color: var(--text-tertiary);
	}
	.primary-btn {
		background: var(--accent-primary);
		color: #000;
		border: none;
		padding: 12px 24px;
		border-radius: 8px;
		font-weight: 800;
		cursor: pointer;
	}

	.analysis-hud {
		margin-top: 24px;
		background: rgba(255, 255, 255, 0.02);
		border: 1px solid rgba(255, 255, 255, 0.05);
		border-radius: 4px;
		padding: 8px 12px;
	}
	.hud-bar {
		height: 2px;
		background: rgba(255, 255, 255, 0.05);
		border-radius: 2px;
		overflow: hidden;
		margin-bottom: 6px;
	}
	.hud-fill {
		height: 100%;
		background: var(--accent-primary);
		transition: width 0.3s ease;
	}
	.hud-label {
		display: flex;
		align-items: center;
		gap: 8px;
		font-size: 9px;
		font-weight: 900;
		color: var(--accent-primary);
		letter-spacing: 0.1em;
	}
</style>
