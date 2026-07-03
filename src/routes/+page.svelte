<script lang="ts">
	import { onMount } from 'svelte';
	import { invoke } from '@tauri-apps/api/core';
	import IntelligenceDossier from '$lib/components/IntelligenceDossier.svelte';
	import Map from '$lib/components/Map.svelte';
	import LinkAnalysis from '$lib/components/LinkAnalysis.svelte';
	import FirstLaunch from '$lib/components/FirstLaunch.svelte';
	import GlobalActions from '$lib/components/dashboard/GlobalActions.svelte';
	import SyncDiffSummary from '$lib/components/dashboard/SyncDiffSummary.svelte';
	import IntelligenceCenter from '$lib/components/IntelligenceCenter.svelte';
	import EvidenceVault from '$lib/components/EvidenceVault.svelte';
	import DownloadAgent from '$lib/components/DownloadAgent.svelte';
	import Settings from '$lib/components/Settings.svelte';
	import AnalysisModal from '$lib/components/AnalysisModal.svelte';
	import IntelligenceModal from '$lib/components/IntelligenceModal.svelte';
	import MediaViewer from '$lib/components/MediaViewer.svelte';
	import Dashboard from '$lib/components/dashboard/Dashboard.svelte';
	import { MODELS } from '$lib/models';
	import type { CaseSummary, RecordPage, RecordSummary, SyncReport } from '$lib/types';
	import { addToast, updateToast } from '$lib/stores/toastStore.svelte';
	import { appStore } from '$lib/stores/appStore.svelte';
	import { intelligenceStore } from '$lib/stores/intelligenceStore.svelte';
	import { settingsStore } from '$lib/stores/settingsStore.svelte';
	import { listen, type UnlistenFn } from '@tauri-apps/api/event';
	import { logger } from '$lib/logger';
	import { Brain, Layers } from 'lucide-svelte';

	import SystemSplash from '$lib/components/layout/SystemSplash.svelte';
	import AmbientBackground from '$lib/components/layout/AmbientBackground.svelte';
	import StatsBar from '$lib/components/layout/StatsBar.svelte';
	import Footer from '$lib/components/layout/Footer.svelte';
	import { discoverWarGovCsvUrl, validateWarGovCsv } from '$lib/warGovSource';

	let isProvisioned = $state(false);

	let records = $state<RecordSummary[]>([]);
	let recordsTotal = $state(0);
	let recordsLimit = $state(250);
	let cases = $state<CaseSummary[]>([]);
	let selectedRecord = $state<RecordSummary | null>(null);
	let selectedCaseId = $state<string | null>(null);

	let query = $state('');
	let busy = $state<string | null>(null);
	let viewMode = $state<'grid' | 'cards' | 'list'>('grid');
	let analysisModalOpen = $state(false);
	let analysisBusy = $state(false);
	let analysisProgress = $state(0);

	let intelligenceModalOpen = $state(false);
	let intelligenceBusy = $state(false);
	let viewerOpen = $state(false);
	let viewerRecord = $state<RecordSummary | null>(null);
	let hasLoaded = $state(false);
	let lastSyncReport = $state<SyncReport | null>(null);

	async function loadInitialData() {
		if (!appStore.initializing) {
			logger.debug('[App] loadInitialData called post-init');
		}
		appStore.initializing = true;

		// Safety timeout: don't stay stuck on splash forever
		const timeoutId = setTimeout(() => {
			if (appStore.initializing) {
				logger.info('[App] loadInitialData is taking too long, forcing splash off.');
				appStore.initializing = false;
				addToast({
					type: 'info',
					message: 'Archive is taking longer than usual to load. System proceeding...',
					duration: 5000
				});
			}
		}, 15000);

		try {
			if (query.trim()) {
				appStore.addBootLog(`Searching for "${query.trim()}"...`);
				const results = await invoke<{ total: number; results: Array<Partial<RecordSummary>> }>(
					'search',
					{
						request: { query: query.trim(), filters: null }
					}
				);
				records = results.results.map((r) => ({
					...r,
					source_type: r.source_type || 'official',
					entity_count: 0,
					incident_date: r.release_date
				})) as RecordSummary[];
				recordsTotal = results.total;
			} else {
				appStore.addBootLog('Fetching archive records...');
				const page = await invoke<RecordPage>('list_records_page', {
					filter: { source_type: null, local_only: null, query: null },
					limit: recordsLimit,
					offset: 0
				});
				records = page.records;
				recordsTotal = page.total;
			}

			// list_cases and the neural-model status refresh are independent of each other and
			// of the records/search fetch above — no need to serialize them (they were made
			// sequential in a924e54 purely to interleave boot-log messages, not for a race).
			appStore.addBootLog('Loading forensic cases and verifying neural models...');
			const [nextCases] = await Promise.all([
				invoke<CaseSummary[]>('list_cases'),
				intelligenceStore.loadStatus()
			]);
			cases = nextCases;
			if (!selectedCaseId && nextCases.length > 0) {
				selectedCaseId = nextCases[0].id;
			}
		} catch (e) {
			logger.error('[App] loadInitialData failed:', e);
			addToast({ type: 'error', message: `Failed to load data: ${e}`, duration: 5000 });
		} finally {
			clearTimeout(timeoutId);
			appStore.initializing = false;
			appStore.addBootLog('Intelligence OS Ready.');
		}
	}

	async function loadMoreRecords() {
		const page = await invoke<RecordPage>('list_records_page', {
			filter: { source_type: null, local_only: null, query: null },
			limit: recordsLimit,
			offset: records.length
		});
		records = [...records, ...page.records];
		recordsTotal = page.total;
	}

	async function sync() {
		busy = 'sync';
		const toastId = addToast({
			type: 'loading',
			message: 'Syncing WAR.gov Database...',
			duration: 0
		});

		// The CSV fetch + sync itself is the only thing that should be reported as "Sync
		// failed" — a failure in a post-sync step (cleanup, auto-download) below means the
		// sync already committed successfully and should be reported as its own thing,
		// not misattributed to the sync.
		let report: SyncReport;
		try {
			const sourcePageUrl = 'https://www.war.gov/UFO/';
			const sourcePage = await fetch(sourcePageUrl, { cache: 'no-store' });
			if (!sourcePage.ok) {
				throw new Error(`WAR.gov UFO page HTTP ${sourcePage.status}: ${sourcePage.statusText}`);
			}
			const sourceHtml = await sourcePage.text();
			const csvUrl = discoverWarGovCsvUrl(sourceHtml);
			const response = await fetch(csvUrl, { cache: 'no-store' });
			if (!response.ok) {
				throw new Error(`HTTP ${response.status}: ${response.statusText}`);
			}
			const csvText = await response.text();
			validateWarGovCsv(csvText);
			report = await invoke<SyncReport>('sync_official_source_with_csv', {
				csv: csvText,
				upstreamUrl: csvUrl
			});
		} catch (e) {
			updateToast(toastId, { type: 'error', message: `Sync failed: ${e}`, duration: 5000 });
			busy = null;
			return;
		}

		try {
			const agentSettings = await invoke<{ auto_sync: boolean; auto_analyze: boolean }>(
				'get_app_settings',
				{ key: 'ingestion_agent' }
			);
			const removed = await invoke<number>('cleanup_duplicates');
			const poisoned = await invoke<number>('cleanup_poisoned_artifacts');
			if (removed > 0 || poisoned > 0) {
				addToast({
					type: 'info',
					message: `Data integrity: Cleaned up ${removed} duplicates and ${poisoned} broken files.`,
					duration: 3000
				});
			}
			await loadInitialData();
			lastSyncReport = report.diffs.length > 0 ? report : null;
			const diffSummary = `${report.added} added, ${report.changed} changed, ${report.removed} removed.`;
			if (agentSettings?.auto_sync) {
				updateToast(toastId, {
					type: 'info',
					message: `Sync complete: ${diffSummary} Auto-retrieval is enabled; downloading missing records...`,
					duration: 4000
				});

				appStore.activeView = 'agent';
				await invoke('download_missing_records');
			} else {
				updateToast(toastId, {
					type: 'success',
					message: `Sync complete: ${diffSummary} Auto-retrieval is disabled.`,
					duration: 4000
				});
			}
		} catch (e) {
			addToast({
				type: 'error',
				message: `Sync succeeded, but post-sync processing failed: ${e}`,
				duration: 5000
			});
		} finally {
			busy = null;
		}
	}

	let systemStats = $state<{
		cpu_usage: number;
		process_memory_mb: number;
	} | null>(null);

	// Auto-detect provisioning
	onMount(() => {
		logger.debug('[App] Mounting +page...');

		(async () => {
			try {
				appStore.addBootLog('Synchronizing Vault...');
				await settingsStore.init();

				appStore.addBootLog('Verifying Neural Environment...');
				// intelligenceStore.init() already fetches hardware diagnostics and model status
				// as part of its own loadStatus() call — re-invoking check_model_status and
				// get_hardware_diagnostics here would just re-fetch what it already has.
				await intelligenceStore.init();

				const specs = intelligenceStore.diagnostics;
				logger.debug('[App] Specs:', specs);
				const tier = specs?.recommended_tier === 'Elite' ? 'Elite' : 'Standard';
				const requiredModels = MODELS[tier];

				const allPresent = requiredModels.every(
					(m) => intelligenceStore.models.find((im) => im.id === m.id)?.status === 'ready'
				);
				logger.debug('[App] All models present:', allPresent);

				if (allPresent) {
					isProvisioned = true;
					// If already provisioned, trigger load immediately
					await loadInitialData();
					// Without this, the $effect below (which also calls loadInitialData once
					// isProvisioned && !hasLoaded && !initializing) fires again right after this
					// completes, running the entire records/cases/status fetch a second time on
					// every normal boot.
					hasLoaded = true;
				} else {
					isProvisioned = false;
					appStore.initializing = false;
				}
			} catch (e) {
				logger.error('Provisioning check failed', e);
				appStore.addBootLog(`Error: ${e}`);
				// Don't leave user stuck on splash if possible
				appStore.initializing = false;
			}
		})();

		const statsInterval = setInterval(async () => {
			try {
				systemStats = await invoke('get_system_stats');
			} catch (e) {
				logger.debug('Failed to poll system stats', e);
			}
		}, 2000);

		let unlistenAnalysis: UnlistenFn;
		listen<{
			record_id?: string;
			status: string;
		}>('analysis-progress', (event) => {
			const payload = event.payload;
			if (payload.record_id) {
				const idx = records.findIndex((r) => r.id === payload.record_id);
				if (idx !== -1) {
					// Map the event status to the record analysis_status
					let newStatus = records[idx].analysis_status;
					if (
						payload.status === 'extracting-foundation' ||
						payload.status === 'processing' ||
						payload.status === 'indexing'
					) {
						newStatus = 'indexing';
					} else if (payload.status === 'foundation-indexed') {
						newStatus = 'indexed';
					} else if (payload.status === 'analyzing' || payload.status === 'synthesizing') {
						newStatus = 'synthesizing';
					} else if (payload.status === 'completed' || payload.status === 'record-completed') {
						newStatus = 'completed';
					} else if (payload.status === 'record-failed') {
						newStatus = 'failed';
					}

					if (newStatus !== records[idx].analysis_status) {
						records[idx] = { ...records[idx], analysis_status: newStatus };
					}
				}
			}
		}).then((u) => (unlistenAnalysis = u));

		return () => {
			clearInterval(statsInterval);
			if (unlistenAnalysis) unlistenAnalysis();
			intelligenceStore.destroy();
		};
	});

	$effect(() => {
		logger.debug('[App] Provisioned/View effect:', {
			isProvisioned: $state.snapshot(isProvisioned),
			hasLoaded: $state.snapshot(hasLoaded),
			initializing: appStore.initializing,
			activeView: $state.snapshot(appStore.activeView)
		});
		if (isProvisioned && !hasLoaded && !appStore.initializing) {
			if (appStore.activeView === 'dashboard') {
				logger.debug('[App] Triggering loadInitialData from effect...');
				hasLoaded = true;
				loadInitialData();
			}
		}
	});

	$effect(() => {
		logger.debug('[App] Active view changed:', $state.snapshot(appStore.activeView));
		// Clear selection when switching top-level modules
		if (appStore.activeView && appStore.activeView !== 'map') {
			selectedRecord = null;
		}
	});

	$effect(() => {
		// get_database_status is expensive (17 COUNT(*) subqueries); only poll it on a 5s
		// interval while the Intelligence Center view is actually showing it.
		if (appStore.activeView === 'intelligence') {
			intelligenceStore.loadStatus();
			intelligenceStore.resumeStatusPolling();
		} else {
			intelligenceStore.pauseStatusPolling();
		}
	});

	$effect(() => {
		const id = appStore.selectedRecordId;
		if (!id || records.length === 0) return;
		const match = records.find((record) => record.id === id);
		if (match) {
			selectedRecord = match;
			appStore.activeView = 'map';
			appStore.selectedRecordId = null;
		} else {
			invoke<RecordSummary | null>('get_record', { id })
				.then((record) => {
					if (record) {
						selectedRecord = record;
						appStore.activeView = 'map';
					}
				})
				.finally(() => {
					appStore.selectedRecordId = null;
				});
		}
	});
</script>

{#if !isProvisioned}
	<FirstLaunch
		onComplete={() => {
			logger.debug('[App] FirstLaunch complete.');
			isProvisioned = true;
			hasLoaded = true;
			loadInitialData();
		}}
	/>
{:else if appStore.initializing}
	<SystemSplash />
{:else}
	<AmbientBackground />

	<div class="os-container glass-panel" class:blur={appStore.initializing}>
		<header class="os-header glass-header" data-tauri-drag-region>
			<div class="view-context" data-tauri-drag-region>
				<h2 class="view-title" data-tauri-drag-region>
					{(appStore.activeView === 'dashboard'
						? 'Evidence Archive'
						: appStore.activeView === 'intelligence'
							? 'Neural Engine'
							: appStore.activeView === 'vault'
								? 'Secure Vault'
								: appStore.activeView === 'agent'
									? 'Ingestion Agent'
									: appStore.activeView
					).toUpperCase()}
				</h2>
			</div>

			<div class="header-actions">
				<GlobalActions
					bind:query
					bind:viewMode
					onLoad={loadInitialData}
					onSelect={(r: RecordSummary) => (selectedRecord = r)}
					onSync={sync}
					onAnalyze={() => (analysisModalOpen = true)}
					bind:busy
				/>
			</div>
		</header>

		<StatsBar />

		{#if lastSyncReport}
			<SyncDiffSummary report={lastSyncReport} onDismiss={() => (lastSyncReport = null)} />
		{/if}

		<div class="os-body">
			<main class="os-main">
				<div class="view-container">
					{#if appStore.activeView === 'dashboard'}
						<Dashboard
							{records}
							libraryPath={intelligenceStore.status?.library_path ?? null}
							{viewMode}
							{cases}
							{selectedCaseId}
							bind:selectedRecord
							onChanged={() => loadInitialData()}
							onAnalyze={() => (analysisModalOpen = true)}
							onSynthesize={() => (intelligenceModalOpen = true)}
							onViewMedia={(r) => {
								viewerRecord = r;
								viewerOpen = true;
							}}
							onSync={sync}
							hasActiveQuery={query.trim().length > 0}
						/>
						{#if !selectedRecord && records.length < recordsTotal}
							<div class="load-more-row">
								<button class="load-more-btn" onclick={loadMoreRecords}>
									Load {Math.min(recordsLimit, recordsTotal - records.length)} more records
								</button>
								<span>{records.length} / {recordsTotal}</span>
							</div>
						{/if}
					{:else if appStore.activeView === 'intelligence'}
						<IntelligenceCenter
							onAnalyze={() => (analysisModalOpen = true)}
							onSynthesize={() => (intelligenceModalOpen = true)}
						/>
					{:else if appStore.activeView === 'vault'}
						<EvidenceVault />
					{:else if appStore.activeView === 'agent'}
						<DownloadAgent
							onComplete={loadInitialData}
							onAnalyze={() => (analysisModalOpen = true)}
						/>
					{:else if appStore.activeView === 'map'}
						{#if selectedRecord}
							<IntelligenceDossier
								record={selectedRecord}
								libraryPath={intelligenceStore.status?.library_path}
								{cases}
								{selectedCaseId}
								onBack={() => (selectedRecord = null)}
								onChanged={() => loadInitialData()}
								onAnalyze={() => (analysisModalOpen = true)}
								onSynthesize={() => (intelligenceModalOpen = true)}
							/>
							{#if records.length < recordsTotal}
								<div class="load-more-row">
									<button class="load-more-btn" onclick={loadMoreRecords}>
										Load {Math.min(recordsLimit, recordsTotal - records.length)} more records
									</button>
									<span>{records.length} / {recordsTotal}</span>
								</div>
							{/if}
						{:else}
							<div class="view-empty">
								<Map {records} onSelect={(r) => (selectedRecord = r)} />
							</div>
						{/if}
					{:else if appStore.activeView === 'link-analysis'}
						<div class="view-empty">
							<LinkAnalysis {records} />
						</div>
					{:else if appStore.activeView === 'settings'}
						<Settings />
					{/if}
				</div>
			</main>
		</div>

		<Footer {systemStats} {busy} />
	</div>

	<AnalysisModal
		bind:isOpen={analysisModalOpen}
		bind:isBusy={analysisBusy}
		bind:progress={analysisProgress}
		onComplete={loadInitialData}
	/>
	<IntelligenceModal
		bind:isOpen={intelligenceModalOpen}
		bind:isBusy={intelligenceBusy}
		onComplete={loadInitialData}
	/>
	{#if viewerRecord}
		<MediaViewer record={viewerRecord} bind:isOpen={viewerOpen} />
	{/if}

	{#if (analysisBusy && !analysisModalOpen) || (intelligenceBusy && !intelligenceModalOpen)}
		<div class="active-pipelines-floating">
			{#if analysisBusy && !analysisModalOpen}
				<button class="pipeline-pill" onclick={() => (analysisModalOpen = true)}>
					<span class="indicator-glow pulse-active yellow"></span>
					<Layers size={14} style="color: var(--accent-primary)" />
					<span class="label">Ingestion In Progress ({analysisProgress.toFixed(0)}%)</span>
				</button>
			{/if}
			{#if intelligenceBusy && !intelligenceModalOpen}
				<button class="pipeline-pill" onclick={() => (intelligenceModalOpen = true)}>
					<span class="indicator-glow pulse-active blue"></span>
					<Brain size={14} style="color: var(--color-accent-info)" />
					<span class="label">Neural Synthesis Active</span>
				</button>
			{/if}
		</div>
	{/if}
{/if}

<style>
	.os-container {
		display: flex;
		flex-direction: column;
		height: 96vh;
		width: 96vw;
		margin: 2vh auto;
		border-radius: var(--radius-lg);
		overflow: hidden;
	}

	.os-header {
		height: 64px;
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 0 32px;
		z-index: 10;
		border-bottom: 1px solid var(--border-subtle);
		user-select: none;
		-webkit-user-select: none;
	}

	.view-context {
		display: flex;
		align-items: center;
	}

	.view-title {
		font-size: var(--text-lg);
		font-weight: 800;
		letter-spacing: 0.15em;
		color: var(--text-secondary);
		margin: 0;
	}

	.header-actions {
		display: flex;
		gap: var(--space-3xl);
		align-items: center;
	}

	.os-body {
		display: flex;
		flex: 1;
		overflow: hidden;
	}

	.os-main {
		flex: 1;
		overflow-y: auto;
		position: relative;
	}

	.view-container {
		height: 100%;
		width: 100%;
	}

	.load-more-row {
		position: absolute;
		left: 50%;
		bottom: 18px;
		transform: translateX(-50%);
		display: flex;
		align-items: center;
		gap: var(--space-xl);
		padding: 8px 12px;
		border: 1px solid var(--border-subtle);
		border-radius: var(--radius-sm);
		background: var(--bg-surface);
		font-size: var(--text-base);
		color: var(--text-secondary);
		z-index: 20;
	}

	.load-more-btn {
		color: var(--accent-primary);
		font-weight: 700;
	}

	.view-empty {
		height: 100%;
		width: 100%;
		box-sizing: border-box;
	}

	.os-container.blur {
		filter: blur(8px);
		pointer-events: none;
	}

	.active-pipelines-floating {
		position: fixed;
		bottom: 24px;
		right: 24px;
		z-index: 1500;
		display: flex;
		flex-direction: column;
		gap: var(--space-lg);
		pointer-events: auto;
		animation: slideIn 0.3s cubic-bezier(0.16, 1, 0.3, 1);
	}

	.pipeline-pill {
		display: flex;
		align-items: center;
		gap: var(--space-lg);
		padding: 10px 16px;
		background: rgba(10, 12, 16, 0.75);
		backdrop-filter: blur(12px);
		-webkit-backdrop-filter: blur(12px);
		border: 1px solid var(--border-subtle);
		border-radius: 30px;
		color: var(--text-primary);
		font-family: var(--font-sans);
		font-size: var(--text-sm);
		font-weight: 600;
		letter-spacing: 0.05em;
		text-transform: uppercase;
		cursor: pointer;
		box-shadow:
			0 4px 20px rgba(0, 0, 0, 0.4),
			inset 0 1px 0 rgba(255, 255, 255, 0.05);
		transition: all 0.2s cubic-bezier(0.16, 1, 0.3, 1);
	}

	.pipeline-pill:hover {
		transform: translateY(-2px);
		border-color: rgba(255, 255, 255, 0.15);
		box-shadow:
			0 6px 24px rgba(0, 0, 0, 0.5),
			inset 0 1px 0 rgba(255, 255, 255, 0.1);
	}

	.indicator-glow {
		width: 6px;
		height: 6px;
		border-radius: 50%;
		display: inline-block;
	}

	.indicator-glow.yellow {
		background: var(--accent-primary);
		box-shadow: 0 0 10px var(--accent-primary);
	}

	.indicator-glow.blue {
		background: var(--color-accent-info);
		box-shadow: 0 0 10px var(--color-accent-info);
	}

	.indicator-glow.pulse-active {
		animation: floating-glow-pulse 1.5s infinite ease-in-out;
	}

	@keyframes floating-glow-pulse {
		0%,
		100% {
			opacity: 0.6;
			transform: scale(1);
		}
		50% {
			opacity: 1;
			transform: scale(1.2);
		}
	}

	@keyframes slideIn {
		from {
			opacity: 0;
			transform: translateY(16px);
		}
		to {
			opacity: 1;
			transform: translateY(0);
		}
	}
</style>
