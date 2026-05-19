<script lang="ts">
	import { onMount } from 'svelte';
	import { invoke } from '@tauri-apps/api/core';
	import IntelligenceDossier from '$lib/components/IntelligenceDossier.svelte';
	import Map from '$lib/components/Map.svelte';
	import LinkAnalysis from '$lib/components/LinkAnalysis.svelte';
	import FirstLaunch from '$lib/components/FirstLaunch.svelte';
	import GlobalActions from '$lib/components/dashboard/GlobalActions.svelte';
	import IntelligenceCenter from '$lib/components/IntelligenceCenter.svelte';
	import EvidenceVault from '$lib/components/EvidenceVault.svelte';
	import DownloadAgent from '$lib/components/DownloadAgent.svelte';
	import Settings from '$lib/components/Settings.svelte';
	import AnalysisModal from '$lib/components/AnalysisModal.svelte';
	import MediaViewer from '$lib/components/MediaViewer.svelte';
	import Dashboard from '$lib/components/dashboard/Dashboard.svelte';
	import { MODELS } from '$lib/models';
	import type { CaseSummary, DatabaseStatus, RecordSummary } from '$lib/types';
	import { addToast, updateToast } from '$lib/toastStore';
	import { activeView, selectedRecordId } from '$lib/store';
	import { listen, type UnlistenFn } from '@tauri-apps/api/event';
	import { logger } from '$lib/logger';
	import { formatBytes } from '$lib/utils';

	let isProvisioned = $state(false);

	let records = $state<RecordSummary[]>([]);
	let cases = $state<CaseSummary[]>([]);
	let selectedRecord = $state<RecordSummary | null>(null);
	let selectedCaseId = $state<string | null>(null);
	let databaseStatus = $state<DatabaseStatus | null>(null);

	let query = $state('');
	let busy = $state<string | null>(null);
	let initializing = $state(true);
	let viewMode = $state<'grid' | 'cards' | 'list'>('grid');
	let analysisModalOpen = $state(false);
	let viewerOpen = $state(false);
	let viewerRecord = $state<RecordSummary | null>(null);
	let hasLoaded = $state(false);

	async function loadInitialData() {
		logger.debug('[App] loadInitialData called');
		initializing = true;
		try {
			if (query.trim()) {
				const results = await invoke<{ results: RecordSummary[] }>('search', {
					request: { query: query.trim(), filters: null }
				});
				records = results.results.map((r) => ({
					...r,
					source_type: r.source_type || 'official',
					entity_count: 0,
					incident_date: r.release_date
				}));
			} else {
				records = await invoke<RecordSummary[]>('list_records', {
					filter: { source_type: null, local_only: null, query: null }
				});
			}

			const [nextCases, nextStatus] = await Promise.all([
				invoke<CaseSummary[]>('list_cases'),
				invoke<DatabaseStatus>('get_database_status')
			]);
			cases = nextCases;
			databaseStatus = nextStatus;
			if (!selectedCaseId && nextCases.length > 0) {
				selectedCaseId = nextCases[0].id;
			}

			if (initializing) await new Promise((resolve) => setTimeout(resolve, 800));
		} catch (e) {
			addToast({ type: 'error', message: `Failed to load data: ${e}`, duration: 5000 });
		} finally {
			initializing = false;
		}
	}

	async function sync() {
		busy = 'sync';
		const toastId = addToast({
			type: 'loading',
			message: 'Syncing WAR.gov Database...',
			duration: 0
		});
		try {
			const response = await fetch('https://www.war.gov/Portals/1/Interactive/2026/UFO/uap-release001.csv');
			if (!response.ok) {
				throw new Error(`HTTP ${response.status}: ${response.statusText}`);
			}
			const csvText = await response.text();
			await invoke('sync_official_source_with_csv', { csv: csvText });
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
			if (agentSettings?.auto_sync) {
				updateToast(toastId, {
					type: 'info',
					message: 'Sync complete. Auto-retrieval is enabled; downloading missing records...',
					duration: 3000
				});

				$activeView = 'agent';
				await invoke('download_missing_records');
			} else {
				updateToast(toastId, {
					type: 'success',
					message: 'Sync complete. Auto-retrieval is disabled.',
					duration: 3000
				});
			}
			busy = null;
		} catch (e) {
			updateToast(toastId, { type: 'error', message: `Sync failed: ${e}`, duration: 5000 });
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
				const modelStatus = await invoke<Record<string, boolean>>('check_model_status');
				const specs = await invoke<{ recommended_tier: 'Standard' | 'Elite' }>('get_hardware_diagnostics');

				logger.debug('[App] Specs:', specs);
				const tier = specs.recommended_tier === 'Elite' ? 'Elite' : 'Standard';
				const requiredModels = MODELS[tier];

				const allPresent = requiredModels.every((m) => modelStatus[m.id]);
				logger.debug('[App] All models present:', allPresent);
				if (allPresent) {
					isProvisioned = true;
					// If already provisioned, trigger load immediately
					loadInitialData();
				}
			} catch (e) {
				console.error('Provisioning check failed', e);
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
		};
	});

	$effect(() => {
		logger.debug('[App] Provisioned/View effect:', { 
			isProvisioned: $state.snapshot(isProvisioned), 
			hasLoaded: $state.snapshot(hasLoaded), 
			initializing: $state.snapshot(initializing), 
			activeView: $state.snapshot($activeView) 
		});
		if (isProvisioned && !hasLoaded && !initializing) {
			if ($activeView === 'dashboard') {
				logger.debug('[App] Triggering loadInitialData from effect...');
				hasLoaded = true;
				loadInitialData();
			}
		}
	});

	$effect(() => {
		logger.debug('[App] Active view changed:', $state.snapshot($activeView));
		// Clear selection when switching top-level modules
		if ($activeView && $activeView !== 'map') {
			selectedRecord = null;
		}
	});

	$effect(() => {
		const id = $selectedRecordId;
		if (!id || records.length === 0) return;
		const match = records.find((record) => record.id === id);
		if (match) {
			selectedRecord = match;
			$activeView = 'map';
			$selectedRecordId = null;
		} else {
			invoke<RecordSummary | null>('get_record', { id })
				.then((record) => {
					if (record) {
						selectedRecord = record;
						$activeView = 'map';
					}
				})
				.finally(() => {
					$selectedRecordId = null;
				});
		}
	});
</script>

{#if !isProvisioned}
	<FirstLaunch
		onComplete={() => {
			logger.debug('[App] FirstLaunch complete.');
			isProvisioned = true;
			loadInitialData();
		}}
	/>
{:else if initializing}
	<div class="system-splash">
		<div class="splash-content">
			<div class="loader-spinner"></div>
			<h2>INTELLIGENCE OS INITIALIZING</h2>
			<p>Syncing local evidence vault and neural models...</p>
			<div class="boot-log">
				<span>[SYSTEM] Mounting secure database...</span>
				<span>[SYSTEM] Initializing vector search engine...</span>
				<span>[SYSTEM] Loading AARO official source records...</span>
			</div>
		</div>
	</div>
{:else}
	<div class="ambient-background">
		<div class="ambient-blob b1"></div>
		<div class="ambient-blob b2"></div>
		<div class="ambient-blob b3"></div>
	</div>

	<div class="os-container glass-panel" class:blur={initializing}>
		<header class="os-header glass-header">
			<div class="view-context">
				<h2 class="view-title">
					{($activeView === 'dashboard'
						? 'Evidence Archive'
						: $activeView === 'intelligence'
							? 'Neural Engine'
							: $activeView === 'vault'
								? 'Secure Vault'
								: $activeView === 'agent'
									? 'Ingestion Agent'
									: $activeView
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

		{#if databaseStatus}
			<div class="stats-bar">
				<span class="stat">Total Records: <strong>{databaseStatus.total_count}</strong></span>
				<span class="stat"
					>Vault Storage: <strong>{formatBytes(databaseStatus.total_size)}</strong></span
				>
				<span class="stat">Database: <strong>Online</strong></span>
			</div>
		{/if}

		<div class="os-body">
			<main class="os-main">
				<div class="view-container">
					{#if $activeView === 'dashboard'}
						<Dashboard
							{records}
							libraryPath={databaseStatus?.library_path ?? null}
							{viewMode}
							{cases}
							{selectedCaseId}
							bind:selectedRecord
							onChanged={() => loadInitialData()}
							onAnalyze={() => (analysisModalOpen = true)}
							onViewMedia={(r) => {
								viewerRecord = r;
								viewerOpen = true;
							}}
						/>
					{:else if $activeView === 'intelligence'}
						<IntelligenceCenter onAnalyze={() => (analysisModalOpen = true)} />
					{:else if $activeView === 'vault'}
						<EvidenceVault />
					{:else if $activeView === 'agent'}
						<DownloadAgent
							onComplete={loadInitialData}
							onAnalyze={() => (analysisModalOpen = true)}
						/>
					{:else if $activeView === 'map'}
						{#if selectedRecord}
							<IntelligenceDossier
								record={selectedRecord}
								libraryPath={databaseStatus?.library_path}
								{cases}
								{selectedCaseId}
								onBack={() => (selectedRecord = null)}
								onChanged={() => loadInitialData()}
								onAnalyze={() => (analysisModalOpen = true)}
							/>
						{:else}
							<div class="view-empty">
								<Map {records} onSelect={(r) => (selectedRecord = r)} />
							</div>
						{/if}
					{:else if $activeView === 'link-analysis'}
						<div class="view-empty">
							<LinkAnalysis {records} />
						</div>
					{:else if $activeView === 'settings'}
						<Settings />
					{/if}
				</div>
			</main>
		</div>

		<footer class="os-footer">
			<div class="f-section">
				<span class="f-label">Ingestion:</span>
				<span class="f-val"
					>{databaseStatus?.local_records || 0} / {databaseStatus?.official_records || 0} Assets</span
				>
			</div>
			<div class="f-section">
				<span class="f-label">Analysis:</span>
				<span class="f-val">{databaseStatus?.analyzed_records || 0} Reports</span>
			</div>
			<div class="f-section resource-monitor">
				{#if systemStats}
					<div class="res-item">
						<span class="f-label">CPU</span>
						<div class="res-bar-wrap">
							<div class="res-bar-fill" style="width: {systemStats.cpu_usage}%"></div>
						</div>
						<span class="f-val">{systemStats.cpu_usage.toFixed(1)}%</span>
					</div>
					<div class="res-item">
						<span class="f-label">MEM</span>
						<span class="f-val">{formatBytes(systemStats.process_memory_mb * 1024 * 1024)}</span>
					</div>
				{/if}
			</div>

			<div class="f-section engine-status">
				<div class="status-orb" class:busy></div>
				<span class="f-val"
					>{busy ? `AGENT ${busy.toUpperCase()} ACTIVE` : 'INTELLIGENCE OS STANDBY'}</span
				>
			</div>
		</footer>
	</div>

	<AnalysisModal bind:isOpen={analysisModalOpen} onComplete={loadInitialData} />
	{#if viewerRecord}
		<MediaViewer record={viewerRecord} bind:isOpen={viewerOpen} />
	{/if}
{/if}

<style>
	.ambient-background {
		position: fixed;
		inset: 0;
		z-index: -1;
		background: #000;
		overflow: hidden;
	}

	.ambient-blob {
		position: absolute;
		border-radius: 50%;
		filter: blur(100px);
		opacity: 0.4;
		animation: ambient-drift 20s infinite alternate cubic-bezier(0.4, 0, 0.2, 1);
	}

	.b1 {
		top: -10%; left: -10%;
		width: 50vw; height: 50vw;
		background: rgba(231, 196, 107, 0.15); /* Accent Primary */
		animation-delay: 0s;
	}
	.b2 {
		bottom: -20%; right: -10%;
		width: 60vw; height: 60vw;
		background: rgba(77, 243, 169, 0.1); /* Accent Success */
		animation-delay: -5s;
		animation-duration: 25s;
	}
	.b3 {
		top: 40%; left: 60%;
		width: 40vw; height: 40vw;
		background: rgba(243, 77, 77, 0.08); /* Accent Danger */
		animation-delay: -10s;
		animation-duration: 30s;
	}

	.os-container {
		display: flex;
		flex-direction: column;
		height: 96vh;
		width: 96vw;
		margin: 2vh auto;
		border-radius: 16px;
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
	}

	.view-context {
		display: flex;
		align-items: center;
	}

	.view-title {
		font-size: 14px;
		font-weight: 800;
		letter-spacing: 0.15em;
		color: var(--text-secondary);
		margin: 0;
	}

	.header-actions {
		display: flex;
		gap: 16px;
		align-items: center;
	}

	.stats-bar {
		display: flex;
		align-items: center;
		gap: 24px;
		padding: 8px 32px;
		background: rgba(0, 0, 0, 0.2);
		border-bottom: 1px solid var(--border-subtle);
		font-size: 11px;
		text-transform: uppercase;
		color: var(--text-secondary);
		letter-spacing: 0.05em;
	}

	.stats-bar strong {
		color: var(--text-primary);
		margin-left: 4px;
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

	.os-footer {
		height: 32px;
		background: #050608;
		border-top: 1px solid var(--border-subtle);
		display: flex;
		align-items: center;
		padding: 0 32px;
		gap: 32px;
		font-size: 10px;
		letter-spacing: 0.1em;
		color: var(--text-tertiary);
		text-transform: uppercase;
	}

	.f-section {
		display: flex;
		gap: 8px;
		align-items: center;
	}

	.resource-monitor {
		margin-left: auto;
		gap: 24px;
		padding-right: 24px;
		border-right: 1px solid var(--border-subtle);
	}

	.res-item {
		display: flex;
		align-items: center;
		gap: 12px;
	}

	.res-bar-wrap {
		width: 40px;
		height: 3px;
		background: rgba(255, 255, 255, 0.05);
		border-radius: 1px;
		overflow: hidden;
	}

	.res-bar-fill {
		height: 100%;
		background: var(--accent-primary);
		transition: width 0.3s ease;
	}

	.f-label {
		opacity: 0.5;
	}

	.f-val {
		color: var(--text-secondary);
		font-weight: 600;
	}

	.engine-status {
		margin-left: auto;
		color: var(--accent-primary);
	}

	.status-orb {
		width: 8px;
		height: 8px;
		border-radius: 50%;
		background: #2a2d35;
	}

	.status-orb.busy {
		background: var(--accent-primary);
		box-shadow: 0 0 8px var(--accent-primary);
		animation: orb-pulse 2s infinite;
	}

	@keyframes orb-pulse {
		0% {
			opacity: 1;
			transform: scale(1);
		}
		50% {
			opacity: 0.5;
			transform: scale(1.2);
		}
		100% {
			opacity: 1;
			transform: scale(1);
		}
	}

	.view-empty {
		height: 100%;
		width: 100%;
		box-sizing: border-box;
	}

	.system-splash {
		position: fixed;
		top: 0;
		left: 0;
		right: 0;
		bottom: 0;
		background: #000;
		z-index: 1000;
		display: flex;
		align-items: center;
		justify-content: center;
	}

	.splash-content {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 24px;
		text-align: center;
	}

	.splash-content h2 {
		font-size: 24px;
		color: var(--text-primary);
		margin: 0;
	}

	.splash-content p {
		color: var(--text-secondary);
		margin: 0;
	}

	.loader-spinner {
		width: 40px;
		height: 40px;
		border: 3px solid rgba(231, 196, 107, 0.1);
		border-top: 3px solid var(--accent-primary);
		border-radius: 50%;
		animation: spin 1s linear infinite;
		margin-bottom: 24px;
	}

	@keyframes spin {
		0% { transform: rotate(0deg); }
		100% { transform: rotate(360deg); }
	}

	.boot-log {
		margin-top: 24px;
		display: flex;
		flex-direction: column;
		gap: 8px;
		font-family: var(--font-mono);
		font-size: 11px;
		color: var(--accent-primary);
		opacity: 0.7;
		text-align: left;
		width: 300px;
	}

	.os-container.blur {
		filter: blur(8px);
		pointer-events: none;
	}
</style>
