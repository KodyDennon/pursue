<script lang="ts">
	import { onMount } from 'svelte';
	import { convertFileSrc, invoke } from '@tauri-apps/api/core';
	import { openPath, openUrl } from '@tauri-apps/plugin-opener';
	import { listen } from '@tauri-apps/api/event';
	import {
		AlertCircle,
		Brain,
		Loader2,
		FileText,
		ImageIcon,
		ChevronLeft,
		Download,
		ExternalLink,
		HardDrive,
		ShieldCheck,
		Activity,
		Terminal,
		Maximize2
	} from 'lucide-svelte';
	import MediaViewer from './MediaViewer.svelte';
	import ForensicAuditViewer from './ForensicAuditViewer.svelte';
	import { addToast } from '$lib/toastStore';
	import { listen, type UnlistenFn } from '@tauri-apps/api/event';
	import type {
		AnalysisReport,
		CaseSummary,
		DownloadResult,
		RecordSummary,
		RecordAsset,
		RecordForensics,
		IntelligenceLog
	} from '$lib/types';

	let {
		record,
		libraryPath = null,
		onBack,
		onChanged,
		onAnalyze
	} = $props<{
		record: RecordSummary;
		libraryPath?: string | null;
		cases: CaseSummary[];
		selectedCaseId: string | null;
		onBack: () => void;
		onChanged: () => void | Promise<void>;
		onAnalyze?: () => void;
	}>();

	function resolvePath(rel: string | null) {
		if (!rel || !libraryPath) return '';
		const cleanLib =
			libraryPath.endsWith('/') || libraryPath.endsWith('\\') ? libraryPath : libraryPath + '/';
		return convertFileSrc(cleanLib + rel);
	}

	let activeTab = $state<'intelligence' | 'forensics' | 'raw' | 'media' | 'case' | 'thoughts'>(
		'intelligence'
	);
	let analysis = $state<AnalysisReport | null>(null);
	let forensics = $state<RecordForensics[]>([]);
	let intelLogs = $state<IntelligenceLog[]>([]);
	let busy = $state<string | null>(null);
	let error = $state<string | null>(null);
	let viewerOpen = $state(false);

	// Real-time analysis status
	let analysisStatus = $state<string | null>(null);
	let analysisProgress = $state(0);

	const images = $derived(
		(analysis?.assets ?? []).filter((a: RecordAsset) => a.asset_type === 'image')
	);

	async function loadAnalysis() {
		if (!record) return;
		error = null;
		try {
			analysis = await invoke<AnalysisReport | null>('get_analysis_result', { id: record.id });

			if (record.analysis_status === 'completed' || record.analysis_status === 'indexed') {
				loadForensics();
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

	async function download() {
		busy = 'download';
		error = null;
		try {
			await invoke<DownloadResult>('download_record', { id: record.id });
			await onChanged();
			await loadAnalysis();
		} catch (e) {
			error = String(e);
		} finally {
			busy = null;
		}
	}

	// The Overhauled Decoupled Analysis Flow
	async function runFullAnalysis() {
		busy = 'analysis';
		error = null;
		analysisStatus = 'Initializing foundation...';
		try {
			if (onAnalyze) onAnalyze();

			// Step 1: Indexing (Foundation - OCR/Vectors)
			analysisStatus = 'Executing OCR & Vectoring...';
			const foundation = await invoke<AnalysisReport>('index_record', { id: record.id });
			analysis = foundation;
			if (onChanged) await onChanged();
			addToast({ type: 'success', message: 'Foundation Indexed - Search Ready', duration: 2000 });

			// Step 2: Synthesis (Deep Intelligence)
			analysisStatus = 'Gemma 4: Synthesizing Intelligence...';
			const synthesis = await invoke<AnalysisReport>('synthesize_intelligence', { id: record.id });
			analysis = synthesis;
			if (onChanged) await onChanged();
			loadForensics();
			addToast({ type: 'success', message: 'Intelligence Synthesis Complete', duration: 3000 });
		} catch (e) {
			error = String(e);
			addToast({ type: 'error', message: `Neural Engine Error: ${e}`, duration: 5000 });
		} finally {
			busy = null;
			analysisStatus = null;
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
			addToast({ type: 'error', message: `System Denied Access: ${e}` });
		} finally {
			busy = null;
		}
	}

	onMount(() => {
		loadAnalysis();

		// Listen for neural progress
		let unlisten: UnlistenFn;
		listen<{
			record_id: string;
			status: string;
			current?: number;
		}>('analysis-progress', (event) => {
			const payload = event.payload;
			if (payload.record_id === record.id) {
				if (payload.status === 'extracting' || payload.status === 'synthesizing') {
					analysisStatus = `Gemma 4: ${payload.status === 'extracting' ? 'Auditing' : 'Synthesizing'}...`;
					if (payload.current) {
						analysisProgress = Math.round((payload.current / 2048) * 100);
					}
				} else if (payload.status === 'loading-model') {
					analysisStatus = 'Waking Neural Engine (15GB)...';
				}
			}
		}).then((u) => (unlisten = u));

		return () => {
			if (unlisten) unlisten();
		};
	});
</script>

<div class="intelligence-dossier glass-panel">
	<header class="dossier-header">
		<button class="back-btn" onclick={onBack}>
			<ChevronLeft size={20} /> Back
		</button>

		<div class="header-main">
			<div class="h-top">
				<span class="source-tag">{record.source_type.toUpperCase()} SOURCE</span>
				{#if record.local_path}
					<span class="status-tag">DOWNLOADED</span>
				{:else}
					<span class="status-tag cloud">REMOTE ASSET</span>
				{/if}
			</div>
			<h2>{record.title}</h2>
			<div class="header-meta">
				<span class="agency">{record.agency || 'AARO'}</span>
				<span class="sep">•</span>
				<span class="type">{record.file_type || 'PDF DOCUMENT'}</span>
				<span class="sep">•</span>
				<span
					class="status"
					class:completed={record.analysis_status === 'completed'}
					class:indexed={record.analysis_status === 'indexed'}
				>
					{record.analysis_status?.toUpperCase() || 'PENDING'}
				</span>
			</div>
		</div>

		<div class="header-actions">
			{#if record.document_url}
				<button class="action-btn" onclick={openSource} title="Open original remote source">
					<ExternalLink size={16} /> Source
				</button>
			{/if}
			{#if record.local_path}
				<button class="action-btn" onclick={revealLocal} disabled={busy === 'open-path'}>
					<HardDrive size={16} /> Local File
				</button>
				<button class="action-btn" onclick={runFullAnalysis} disabled={!!busy}>
					{#if busy === 'analysis'}
						<Loader2 size={16} class="spin" />
					{:else}
						<Brain size={16} /> Analysis
					{/if}
				</button>
				<button class="action-btn primary" onclick={() => (viewerOpen = true)}>
					<Maximize2 size={16} /> View Evidence
				</button>
			{:else}
				<button class="action-btn primary" onclick={download} disabled={!!busy}>
					<Download size={16} /> Download Evidence
				</button>
			{/if}
		</div>

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
		<button
			class:active={activeTab === 'intelligence'}
			onclick={() => (activeTab = 'intelligence')}
		>
			<Brain size={16} /> Executive Intel
		</button>
		<button class:active={activeTab === 'forensics'} onclick={() => (activeTab = 'forensics')}>
			<ShieldCheck size={16} /> Forensic Audit
		</button>
		<button class:active={activeTab === 'thoughts'} onclick={() => (activeTab = 'thoughts')}>
			<Terminal size={16} /> Neural Thoughts
		</button>
		<button class:active={activeTab === 'raw'} onclick={() => (activeTab = 'raw')}>
			<FileText size={16} /> Raw Foundation
		</button>
		<button class:active={activeTab === 'media'} onclick={() => (activeTab = 'media')}>
			<ImageIcon size={16} /> Media Assets
		</button>
	</nav>

	<div class="dossier-body">
		{#if error}
			<div class="error-msg">
				<AlertCircle size={18} />
				<span>Neural Failure: {error}</span>
				<button onclick={() => (error = null)}>Reset</button>
			</div>
		{/if}

		<div class="tab-content">
			{#if activeTab === 'intelligence'}
				<div class="intel-view custom-scrollbar">
					{#if record.intelligence_json}
						{@const intel = JSON.parse(record.intelligence_json)}
						<div class="intel-grid">
							<div class="intel-main-flow">
								<div class="intel-card-section">
									<header class="section-head">
										<span class="prefix">EXECUTIVE SUMMARY</span>
									</header>
									<p class="summary-para">
										{intel.object_description ||
											'Intelligence fragment: Unstructured extraction required.'}
									</p>
								</div>

								<div class="forensic-data-grid">
									<div class="f-data-card">
										<span class="f-label">INCIDENT DATE</span>
										<span class="f-val"
											>{intel.incident_date || record.incident_date || 'UNDEFINED'}</span
										>
									</div>
									<div class="f-data-card">
										<span class="f-label">TARGET LOCATION</span>
										<span class="f-val"
											>{intel.location || record.incident_location || 'GLOBAL'}</span
										>
									</div>
									<div class="f-data-card full">
										<span class="f-label">AGENCY ASSOCIATIONS</span>
										<div class="f-tags">
											{#each intel.agencies || [] as agency (agency)}
												<span class="f-tag">{agency}</span>
											{/each}
											{#if !intel.agencies?.length}
												<span class="f-val-muted">None Logged</span>
											{/if}
										</div>
									</div>
								</div>

								<div class="intel-card-section">
									<header class="section-head">
										<span class="prefix">NEURAL OBSERVATIONS</span>
									</header>
									<p class="observations-para">
										{intel.pilot_observations || 'No qualitative sensor observations resolved.'}
									</p>
								</div>
							</div>

							<aside class="intel-meta-sidebar">
								<div class="fidelity-card">
									<span class="f-label">SYNTHESIS FIDELITY</span>
									<div class="fidelity-dial">
										<svg viewBox="0 0 100 100">
											<circle
												cx="50"
												cy="50"
												r="45"
												fill="none"
												stroke="rgba(255,255,255,0.05)"
												stroke-width="4"
											/>
											<circle
												cx="50"
												cy="50"
												r="45"
												fill="none"
												stroke="var(--accent-primary)"
												stroke-width="4"
												stroke-dasharray="{Math.round((intel.intelligence_score || 0.6) * 283)} 283"
												stroke-linecap="round"
											/>
										</svg>
										<span class="f-percent"
											>{Math.round((intel.intelligence_score || 0.6) * 100)}%</span
										>
									</div>
								</div>

								{#if images.length > 0}
									<div class="multimodal-reference">
										<span class="f-label">MULTIMODAL REF</span>
										<div class="m-grid">
											{#each images.slice(0, 4) as img (img.id)}
												<div class="m-thumb">
													<img src={convertFileSrc(img.local_path)} alt="Visual Intelligence" />
												</div>
											{/each}
										</div>
										<p class="m-caption">Cross-referencing {images.length} visual pattern(s).</p>
									</div>
								{/if}
							</aside>
						</div>
					{:else if record.analysis_status === 'indexed'}
						<div class="pending-intel">
							<Activity size={48} class="accent-icon pulse" />
							<h3>Foundation Indexed</h3>
							<p>Keyword search and metadata foundation is ready. Synthesis is pending.</p>
							<button class="analyze-btn" onclick={runFullAnalysis} disabled={busy === 'analysis'}>
								Synthesize Deep Intelligence
							</button>
						</div>
					{:else}
						<div class="pending-intel">
							<Brain size={48} class="accent-icon" />
							<h3>Intelligence Extraction Pending</h3>
							<p>Initiate Gemma 4 deep analysis to populate this dossier.</p>
							<button class="analyze-btn" onclick={runFullAnalysis} disabled={busy === 'analysis'}>
								{#if busy === 'analysis'}
									<Loader2 size={16} class="spin" /> Initializing...
								{:else}
									Run Full Tactical Analysis
								{/if}
							</button>
						</div>
					{/if}
				</div>
			{:else if activeTab === 'forensics'}
				<div class="forensic-view-container">
					<ForensicAuditViewer recordId={record.id} {forensics} {images} />
				</div>
			{:else if activeTab === 'thoughts'}
				<div class="thoughts-view custom-scrollbar">
					<header class="section-head">
						<span class="prefix">NEURAL THOUGHT LOGS</span>
					</header>
					{#if intelLogs.length > 0}
						<div class="thoughts-stack">
							{#each intelLogs as log (log.id)}
								<div class="thought-entry">
									<div class="thought-meta">
										<span class="t-model">{log.model_id.split('/').pop()}</span>
										<span class="t-date">{new Date(log.created_at).toLocaleString()}</span>
									</div>
									<div class="thought-block">
										<span class="t-label">SYSTEM_PROMPT:</span>
										<pre>{log.system_prompt}</pre>
										<span class="t-label">RAW_SYNTHESIS:</span>
										<pre>{log.response_json}</pre>
									</div>
								</div>
							{/each}
						</div>
					{:else}
						<div class="pending-intel">
							<Terminal size={48} class="accent-icon" />
							<h3>No Thought Logs</h3>
							<p>Neural monologues are captured during deep synthesis.</p>
						</div>
					{/if}
				</div>
			{:else if activeTab === 'raw'}
				<div class="raw-view custom-scrollbar">
					{#if analysis?.ocr_text}
						<div class="ocr-content">
							<header class="section-head">
								<span class="prefix">FOUNDATION OCR LOG</span>
							</header>
							<div class="text-blob">
								{analysis.ocr_text}
							</div>
						</div>
					{:else}
						<div class="pending-intel">
							<FileText size={48} class="accent-icon" />
							<h3>No Foundation Data</h3>
							<p>Run Tactical Analysis to initiate indexing.</p>
						</div>
					{/if}
				</div>
			{:else if activeTab === 'media'}
				<div class="media-view custom-scrollbar">
					{#if images.length > 0}
						<div class="asset-grid">
							{#each images as asset (asset.id)}
								<div class="asset-card">
									<img src={resolvePath(asset.local_path)} alt="Evidence" />
									<div class="asset-info">
										<span class="a-name">{asset.local_path.split('/').pop()}</span>
										<span class="a-type">{asset.mime_type || 'image/png'}</span>
									</div>
								</div>
							{/each}
						</div>
					{:else}
						<div class="pending-intel">
							<ImageIcon size={48} class="accent-icon" />
							<h3>No Extracted Assets</h3>
							<p>Segment media from source documents during analysis.</p>
						</div>
					{/if}
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
		background: #0a0b0d;
	}

	.dossier-header {
		padding: 32px;
		border-bottom: 1px solid var(--border-subtle);
		position: relative;
	}

	.back-btn {
		background: none;
		border: none;
		color: var(--text-tertiary);
		display: flex;
		align-items: center;
		gap: 8px;
		font-size: 13px;
		font-weight: 700;
		margin-bottom: 24px;
		cursor: pointer;
	}

	.header-main h2 {
		font-size: 24px;
		margin: 12px 0;
		color: var(--text-primary);
	}

	.h-top {
		display: flex;
		gap: 12px;
	}
	.source-tag {
		font-size: 10px;
		font-weight: 900;
		letter-spacing: 0.1em;
		color: var(--accent-primary);
	}
	.status-tag {
		font-size: 10px;
		font-weight: 900;
		color: var(--accent-success);
	}
	.status-tag.cloud {
		color: #3296ff;
	}

	.header-meta {
		display: flex;
		align-items: center;
		gap: 12px;
		font-size: 12px;
		color: var(--text-tertiary);
	}
	.status.completed {
		color: var(--accent-success);
		font-weight: 800;
	}
	.status.indexed {
		color: #3296ff;
		font-weight: 800;
	}

	.header-actions {
		display: flex;
		gap: 12px;
		margin-top: 24px;
	}
	.action-btn {
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 8px 16px;
		background: rgba(255, 255, 255, 0.03);
		border: 1px solid var(--border-subtle);
		border-radius: 6px;
		color: var(--text-secondary);
		font-size: 13px;
		font-weight: 600;
		cursor: pointer;
	}
	.action-btn.primary {
		background: var(--accent-primary);
		color: #000;
		border: none;
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
	.hud-label .pct {
		margin-left: auto;
		color: var(--text-tertiary);
	}

	.dossier-tabs {
		display: flex;
		padding: 0 32px;
		gap: 24px;
		border-bottom: 1px solid var(--border-subtle);
		overflow-x: auto;
	}

	.dossier-tabs button {
		padding: 16px 0;
		background: none;
		border: none;
		border-bottom: 2px solid transparent;
		color: var(--text-tertiary);
		font-size: 12px;
		font-weight: 600;
		display: flex;
		align-items: center;
		gap: 10px;
		cursor: pointer;
		white-space: nowrap;
	}

	.dossier-tabs button.active {
		color: var(--accent-primary);
		border-bottom-color: var(--accent-primary);
	}

	.dossier-body {
		flex: 1;
		overflow: hidden;
		position: relative;
	}
	.tab-content {
		height: 100%;
	}

	.intel-view {
		padding: 32px;
		height: 100%;
	}
	.intel-grid {
		display: grid;
		grid-template-columns: 1fr 220px;
		gap: 40px;
	}
	.section-head {
		display: flex;
		align-items: center;
		gap: 12px;
		margin-bottom: 16px;
		border-bottom: 1px solid rgba(255, 255, 255, 0.05);
		padding-bottom: 8px;
	}
	.prefix {
		font-size: 10px;
		font-weight: 900;
		letter-spacing: 0.2em;
		color: var(--text-tertiary);
	}
	.summary-para,
	.observations-para {
		font-size: 14px;
		line-height: 1.6;
		color: var(--text-primary);
		margin: 0;
	}

	.forensic-data-grid {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: 16px;
		margin: 32px 0;
	}
	.f-data-card {
		background: rgba(255, 255, 255, 0.02);
		border: 1px solid var(--border-subtle);
		padding: 16px;
		border-radius: 8px;
		display: flex;
		flex-direction: column;
		gap: 4px;
	}
	.f-data-card.full {
		grid-column: span 2;
	}
	.f-label {
		font-size: 9px;
		font-weight: 800;
		color: var(--text-tertiary);
		letter-spacing: 0.1em;
	}
	.f-val {
		font-size: 14px;
		font-weight: 600;
		color: var(--text-primary);
	}
	.f-val-muted {
		font-size: 12px;
		font-style: italic;
		color: var(--text-tertiary);
	}
	.f-tags {
		display: flex;
		flex-wrap: wrap;
		gap: 8px;
		margin-top: 8px;
	}
	.f-tag {
		background: var(--accent-primary);
		color: #000;
		font-size: 9px;
		font-weight: 900;
		padding: 2px 8px;
		border-radius: 4px;
	}

	.fidelity-card {
		text-align: center;
		margin-bottom: 40px;
	}
	.fidelity-dial {
		position: relative;
		width: 100px;
		height: 100px;
		margin: 20px auto;
	}
	.forensic-view-container {
		height: 100%;
		overflow: hidden;
	}

	.thoughts-view {
		padding: 32px;
		height: 100%;
		overflow-y: auto;
	}
	.thoughts-stack {
		display: flex;
		flex-direction: column;
		gap: 24px;
	}
	.thought-entry {
		background: rgba(0, 0, 0, 0.3);
		border: 1px solid var(--border-subtle);
		border-radius: 8px;
		overflow: hidden;
	}
	.thought-meta {
		padding: 8px 16px;
		background: rgba(255, 255, 255, 0.03);
		border-bottom: 1px solid var(--border-subtle);
		display: flex;
		justify-content: space-between;
		font-size: 10px;
		font-family: var(--font-mono);
	}
	.thought-block {
		padding: 16px;
		display: flex;
		flex-direction: column;
		gap: 12px;
	}
	.t-label {
		font-size: 9px;
		font-weight: 900;
		color: var(--accent-primary);
	}
	.thought-block pre {
		font-family: var(--font-mono);
		font-size: 11px;
		color: var(--text-secondary);
		white-space: pre-wrap;
		margin: 0;
		background: rgba(255, 255, 255, 0.02);
		padding: 12px;
		border-radius: 4px;
	}

	.raw-view {
		padding: 32px;
		height: 100%;
		overflow-y: auto;
	}
	.text-blob {
		font-family: var(--font-mono);
		font-size: 12px;
		line-height: 1.8;
		color: var(--text-secondary);
		white-space: pre-wrap;
	}

	.media-view {
		padding: 32px;
		height: 100%;
	}
	.asset-grid {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
		gap: 16px;
	}
	.asset-card {
		background: rgba(255, 255, 255, 0.02);
		border: 1px solid var(--border-subtle);
		border-radius: 8px;
		overflow: hidden;
	}
	.asset-card img {
		width: 100%;
		aspect-ratio: 16/9;
		object-fit: cover;
	}
	.asset-info {
		padding: 12px;
		display: flex;
		flex-direction: column;
		gap: 4px;
	}
	.a-name {
		font-size: 12px;
		font-weight: 600;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}
	.a-type {
		font-size: 10px;
		color: var(--text-tertiary);
	}

	.pending-intel {
		height: 100%;
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		text-align: center;
		gap: 16px;
	}
	.analyze-btn {
		background: var(--accent-primary);
		color: #000;
		border: none;
		padding: 12px 24px;
		border-radius: 8px;
		font-weight: 800;
		cursor: pointer;
	}

	.error-msg {
		display: flex;
		align-items: center;
		gap: 12px;
		background: rgba(243, 77, 77, 0.1);
		border: 1px solid rgba(243, 77, 77, 0.2);
		padding: 12px 24px;
		color: #ff4d4d;
		font-size: 13px;
	}
	.error-msg button {
		background: none;
		border: none;
		color: #fff;
		text-decoration: underline;
		cursor: pointer;
	}

	:global(.pulse) {
		animation: pulse 2s ease-in-out infinite;
	}
	@keyframes pulse {
		0%,
		100% {
			opacity: 0.5;
		}
		50% {
			opacity: 1;
		}
	}
	:global(.spin) {
		animation: spin 1s linear infinite;
	}
	@keyframes spin {
		from {
			transform: rotate(0deg);
		}
		to {
			transform: rotate(360deg);
		}
	}
</style>
