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
	import IntelligenceModal from '$lib/components/IntelligenceModal.svelte';
	import MediaViewer from '$lib/components/MediaViewer.svelte';
	import Dashboard from '$lib/components/dashboard/Dashboard.svelte';
	import { MODELS } from '$lib/models';
	import type { CaseSummary, DatabaseStatus, RecordSummary } from '$lib/types';
	import { addToast, updateToast } from '$lib/toastStore';
	import { activeView, selectedRecordId } from '$lib/store';
	import { listen, type UnlistenFn } from '@tauri-apps/api/event';
	import { logger } from '$lib/logger';
	import { Brain, Layers } from 'lucide-svelte';

	import SystemSplash from '$lib/components/layout/SystemSplash.svelte';
	import AmbientBackground from '$lib/components/layout/AmbientBackground.svelte';
	import StatsBar from '$lib/components/layout/StatsBar.svelte';
	import Footer from '$lib/components/layout/Footer.svelte';

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
	let analysisBusy = $state(false);
	let analysisProgress = $state(0);

	let intelligenceModalOpen = $state(false);
	let intelligenceBusy = $state(false);
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
	<SystemSplash />
{:else}
	<AmbientBackground />

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

		<StatsBar {databaseStatus} />

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
							onSynthesize={() => (intelligenceModalOpen = true)}
							onViewMedia={(r) => {
								viewerRecord = r;
								viewerOpen = true;
							}}
						/>
					{:else if $activeView === 'intelligence'}
						<IntelligenceCenter
							onAnalyze={() => (analysisModalOpen = true)}
							onSynthesize={() => (intelligenceModalOpen = true)}
						/>
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
								onSynthesize={() => (intelligenceModalOpen = true)}
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

		<Footer {databaseStatus} {systemStats} {busy} />
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
					<Brain size={14} style="color: #50b3ff" />
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
		gap: 10px;
		pointer-events: auto;
		animation: slideIn 0.3s cubic-bezier(0.16, 1, 0.3, 1);
	}

	.pipeline-pill {
		display: flex;
		align-items: center;
		gap: 10px;
		padding: 10px 16px;
		background: rgba(10, 12, 16, 0.75);
		backdrop-filter: blur(12px);
		-webkit-backdrop-filter: blur(12px);
		border: 1px solid var(--border-subtle);
		border-radius: 30px;
		color: var(--text-primary);
		font-family: var(--font-sans);
		font-size: 11px;
		font-weight: 600;
		letter-spacing: 0.05em;
		text-transform: uppercase;
		cursor: pointer;
		box-shadow: 0 4px 20px rgba(0, 0, 0, 0.4), inset 0 1px 0 rgba(255, 255, 255, 0.05);
		transition: all 0.2s cubic-bezier(0.16, 1, 0.3, 1);
	}

	.pipeline-pill:hover {
		transform: translateY(-2px);
		border-color: rgba(255, 255, 255, 0.15);
		box-shadow: 0 6px 24px rgba(0, 0, 0, 0.5), inset 0 1px 0 rgba(255, 255, 255, 0.1);
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
		background: #50b3ff;
		box-shadow: 0 0 10px #50b3ff;
	}

	.indicator-glow.pulse-active {
		animation: floating-glow-pulse 1.5s infinite ease-in-out;
	}

	@keyframes floating-glow-pulse {
		0%, 100% { opacity: 0.6; transform: scale(1); }
		50% { opacity: 1; transform: scale(1.2); }
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
