<script lang="ts">
	import {
		ChevronLeft,
		Brain,
		Layers,
		Fingerprint,
		Database,
		Clock,
		ExternalLink,
		HardDrive,
		Search,
		Download,
		Maximize2,
		AlertCircle,
		Activity
	} from 'lucide-svelte';
	import type { RecordSummary, AnalysisReport } from '$lib/types';

	let {
		record,
		analysis,
		activeDomain = $bindable(),
		isIndexed,
		isSynthesisOutdated,
		analysisStatus,
		analysisProgress,
		busy,
		onBack,
		openSource,
		revealLocal,
		runFoundationIndexing,
		runDeepSynthesis,
		download,
		setViewerOpen
	} = $props<{
		record: RecordSummary;
		analysis: AnalysisReport | null;
		activeDomain: 'intelligence' | 'foundation';
		isIndexed: boolean;
		isSynthesisOutdated: boolean;
		analysisStatus: string | null;
		analysisProgress: number;
		busy: string | null;
		onBack: () => void;
		openSource: () => void;
		revealLocal: () => void;
		runFoundationIndexing: () => void;
		runDeepSynthesis: () => void;
		download: () => void;
		setViewerOpen: (open: boolean) => void;
	}>();
</script>

<header class="dossier-header">
	<div class="h-nav">
		<button class="back-btn" onclick={onBack}>
			<ChevronLeft size={20} /> Back to Archive
		</button>

		<div class="domain-selector">
			<button class:active={activeDomain === 'intelligence'} onclick={() => (activeDomain = 'intelligence')}>
				<Brain size={14} /> INTELLIGENCE
			</button>
			<button class:active={activeDomain === 'foundation'} onclick={() => (activeDomain = 'foundation')}>
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

			<button
				class="btn-premium"
				class:accent={!isIndexed}
				onclick={runFoundationIndexing}
				disabled={!!busy}
			>
				{#if isIndexed}
					<Layers size={14} /> Re-index Foundation
				{:else}
					<Search size={14} /> Audit Index
				{/if}
			</button>
			{#if isIndexed}
				<button class="btn-premium accent" onclick={runDeepSynthesis} disabled={!!busy}>
					<Brain size={14} /> Neural Synthesis
				</button>
			{/if}

			<button class="btn-premium" onclick={() => setViewerOpen(true)}>
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

<style>
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
	.source-tag,
	.engine-tag {
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
	.status-tag.success {
		color: var(--accent-success);
	}
	.status-tag.cloud {
		color: #3296ff;
	}

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

	:global(.pulse) {
		animation: pulse-light 1.5s infinite ease-in-out;
	}

	@keyframes pulse-light {
		0%,
		100% {
			opacity: 0.6;
		}
		50% {
			opacity: 1;
			transform: scale(1.05);
		}
	}
</style>
