<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { activeView } from "$lib/store";
  import IntelligenceDossier from "$lib/components/IntelligenceDossier.svelte";
  import Map from "$lib/components/Map.svelte";
  import LinkAnalysis from "$lib/components/LinkAnalysis.svelte";
  import FirstLaunch from "$lib/components/FirstLaunch.svelte";
  import GlobalActions from "$lib/components/dashboard/GlobalActions.svelte";
  import GridView from "$lib/components/dashboard/GridView.svelte";
  import IntelCardsView from "$lib/components/dashboard/IntelCardsView.svelte";
  import DownloadAgent from "$lib/components/DownloadAgent.svelte";
  import IntelligenceCenter from "$lib/components/IntelligenceCenter.svelte";
  import EmptyState from "$lib/components/dashboard/EmptyState.svelte";
  import Settings from "$lib/components/Settings.svelte";
  import type { CaseSummary, DatabaseStatus, RecordSummary } from "$lib/types";
  import { addToast, updateToast } from "$lib/toastStore";

  let isProvisioned = $state(false);
  
  let records = $state<RecordSummary[]>([]);
  let cases = $state<CaseSummary[]>([]);
  let selectedRecord = $state<RecordSummary | null>(null);
  let selectedCaseId = $state<string | null>(null);
  let databaseStatus = $state<DatabaseStatus | null>(null);
  
  let query = $state("");
  let busy = $state<string | null>(null);
  let initializing = $state(true);
  let sidebarWidth = $state(540);
  let isResizing = $state(false);

  function startResizing() {
    isResizing = true;
    document.addEventListener("mousemove", handleResize);
    document.addEventListener("mouseup", stopResizing);
  }

  function handleResize(e: MouseEvent) {
    if (!isResizing) return;
    sidebarWidth = Math.max(400, Math.min(window.innerWidth - 300, window.innerWidth - e.clientX));
  }

  function stopResizing() {
    isResizing = false;
    document.removeEventListener("mousemove", handleResize);
    document.removeEventListener("mouseup", stopResizing);
  }

  async function loadInitialData() {
    initializing = true;
    try {
      const [nextRecords, nextCases, nextStatus] = await Promise.all([
        invoke<RecordSummary[]>("list_records", { filter: { source_type: null, local_only: null, query: query.trim() || null } }),
        invoke<CaseSummary[]>("list_cases"),
        invoke<DatabaseStatus>("get_database_status"),
      ]);
      records = nextRecords;
      cases = nextCases;
      databaseStatus = nextStatus;
      if (!selectedCaseId && nextCases.length > 0) {
        selectedCaseId = nextCases[0].id;
      }
      
      // Artificial delay on first load to show system legit status if requested
      if (initializing) await new Promise(resolve => setTimeout(resolve, 800));
    } catch (e) {
      addToast({ type: "error", message: `Failed to load data: ${e}`, duration: 5000 });
    } finally {
      initializing = false;
    }
  }

  async function sync() {
    busy = "sync";
    const toastId = addToast({ type: "loading", message: "Syncing WAR.gov Database...", duration: 0 });
    try {
      await invoke("sync_official_source");
      await loadInitialData();
      updateToast(toastId, { type: "success", message: "Sync complete!", duration: 3000 });
    } catch (e) {
      updateToast(toastId, { type: "error", message: `Sync failed: ${e}`, duration: 5000 });
    } finally {
      busy = null;
    }
  }

  function formatBytes(value: number | null | undefined) {
    if (!value) return "0 B";
    const units = ["B", "KB", "MB", "GB", "TB"];
    let next = value;
    let unit = 0;
    while (next >= 1024 && unit < units.length - 1) {
      next /= 1024;
      unit += 1;
    }
    return `${next.toFixed(next >= 10 || unit === 0 ? 0 : 1)} ${units[unit]}`;
  }

  onMount(() => {
    if (isProvisioned) void loadInitialData();
  });

  $effect(() => {
    if (isProvisioned) void loadInitialData();
  });
</script>

{#if !isProvisioned}
  <FirstLaunch onComplete={() => (isProvisioned = true)} />
{:else}
  {#if initializing}
    <div class="system-splash">
      <div class="splash-content">
        <Loader2 class="spin" size={48} />
        <h2>Intelligence OS Initializing</h2>
        <p>Syncing local evidence vault and intelligence models...</p>
        <div class="boot-log">
          <span>[SYSTEM] Mounting secure database...</span>
          <span>[SYSTEM] Initializing vector search engine...</span>
          <span>[SYSTEM] Loading AARO official source records...</span>
        </div>
      </div>
    </div>
  {/if}

  <div class="os-container" class:blur={initializing}>
    <header class="os-header glass-header">
      <div class="view-toggles">
        <button class:active={$activeView === 'grid'} onclick={() => $activeView = 'grid'}>Grid</button>
        <button class:active={$activeView === 'cards'} onclick={() => $activeView = 'cards'}>Intel Cards</button>
        <button class:active={$activeView === 'map'} onclick={() => $activeView = 'map'}>Tactical Map</button>
      </div>

      <GlobalActions bind:query bind:busy onLoad={loadInitialData} onSelect={(r) => (selectedRecord = r)} onSync={sync} />
    </header>

    {#if databaseStatus}
      <div class="stats-bar">
        <span class="stat">Total Records: <strong>{databaseStatus.total_records}</strong></span>
        <span class="stat">Vector DB Size: <strong>{formatBytes(databaseStatus.artifact_bytes)}</strong></span>
        <span class="stat">Local DB: <strong>Online</strong></span>
      </div>
    {/if}

    <div class="os-body">
      <main class="os-main">
        {#if records.length === 0 && !query}
          <EmptyState onSync={sync} />
        {:else}
          {#if $activeView === 'grid'}
            <GridView {records} selectedRecordId={selectedRecord?.id} onSelect={(r) => (selectedRecord = r)} />
          {:else if $activeView === 'cards'}
            <IntelCardsView {records} selectedRecordId={selectedRecord?.id} onSelect={(r) => (selectedRecord = r)} />
          {:else if $activeView === 'map'}
            <div class="map-view">
              <Map {records} onSelect={(r) => (selectedRecord = r)} />
            </div>
          {:else if $activeView === 'link-analysis'}
            <div class="link-view">
              <LinkAnalysis {records} />
            </div>
          {:else if $activeView === 'agent'}
            <div class="agent-view">
              <DownloadAgent onComplete={loadInitialData} />
            </div>
          {:else if $activeView === 'intelligence'}
            <IntelligenceCenter />
          {:else if $activeView === 'settings'}
            <div class="settings-view">
              <Settings />
            </div>
          {/if}
        {/if}
      </main>

      {#if selectedRecord}
        <div 
          class="sidebar-resizer" 
          onmousedown={startResizing}
          class:active={isResizing}
        ></div>
        <aside class="os-sidebar" style="width: {sidebarWidth}px; min-width: {sidebarWidth}px;">
          <IntelligenceDossier 
            record={selectedRecord} 
            cases={cases}
            selectedCaseId={selectedCaseId}
            onBack={() => (selectedRecord = null)}
            onChanged={() => loadInitialData()}
          />
        </aside>
      {/if}
    </div>

    <footer class="os-footer">
      <div class="f-section">
        <span class="f-label">Collection Agent:</span>
        <span class="f-val">{databaseStatus?.local_records || 0} / {databaseStatus?.official_records || 0} Synchronized</span>
      </div>
      <div class="f-section">
        <span class="f-label">Intelligence Yield:</span>
        <span class="f-val">{databaseStatus?.analyzed_records || 0} Neural Reports</span>
      </div>
      <div class="f-section engine-status">
        <div class="status-orb" class:busy={busy}></div>
        <span class="f-val">{busy ? `AGENT ${busy.toUpperCase()} ACTIVE` : 'INTELLIGENCE OS STANDBY'}</span>
      </div>
    </footer>
  </div>
{/if}

<style>
  .os-container {
    display: flex;
    flex-direction: column;
    height: 100%;
    width: 100%;
  }

  .os-header {
    height: 64px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 24px;
    z-index: 10;
  }

  .view-toggles {
    display: flex;
    gap: 8px;
    background: rgba(0,0,0,0.4);
    padding: 4px;
    border-radius: var(--radius-md);
    border: 1px solid var(--border-subtle);
  }

  .view-toggles button {
    padding: 6px 16px;
    border-radius: var(--radius-sm);
    font-size: 13px;
    color: var(--text-secondary);
    transition: var(--transition-fast);
  }

  .view-toggles button.active {
    background: var(--bg-surface-elevated);
    color: var(--text-primary);
    box-shadow: 0 2px 8px rgba(0,0,0,0.2);
  }

  .stats-bar {
    display: flex;
    align-items: center;
    gap: 24px;
    padding: 8px 24px;
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

  .os-sidebar {
    height: 100%;
    background: var(--bg-surface-elevated);
    border-left: 1px solid var(--border-subtle);
    position: relative;
  }

  .sidebar-resizer {
    width: 4px;
    height: 100%;
    cursor: col-resize;
    background: transparent;
    transition: background 0.2s;
    z-index: 100;
  }

  .sidebar-resizer:hover, .sidebar-resizer.active {
    background: var(--accent-primary);
  }

  .os-footer {
    height: 32px;
    background: #050608;
    border-top: 1px solid var(--border-subtle);
    display: flex;
    align-items: center;
    padding: 0 24px;
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
    0% { opacity: 1; transform: scale(1); }
    50% { opacity: 0.5; transform: scale(1.2); }
    100% { opacity: 1; transform: scale(1); }
  }

  .map-view, .link-view, .agent-view {
    height: 100%;
    width: 100%;
    padding: 24px;
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
