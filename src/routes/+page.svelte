<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import IntelligenceDossier from "$lib/components/IntelligenceDossier.svelte";
  import Map from "$lib/components/Map.svelte";
  import LinkAnalysis from "$lib/components/LinkAnalysis.svelte";
  import FirstLaunch from "$lib/components/FirstLaunch.svelte";
  import GlobalActions from "$lib/components/dashboard/GlobalActions.svelte";
  import GridView from "$lib/components/dashboard/GridView.svelte";
  import IntelCardsView from "$lib/components/dashboard/IntelCardsView.svelte";
  import ListView from "$lib/components/dashboard/ListView.svelte";
  import IntelligenceCenter from "$lib/components/IntelligenceCenter.svelte";
  import EvidenceVault from "$lib/components/EvidenceVault.svelte";
  import DownloadAgent from "$lib/components/DownloadAgent.svelte";
  import Settings from "$lib/components/Settings.svelte";
  import AnalysisModal from "$lib/components/AnalysisModal.svelte";
  import type { CaseSummary, DatabaseStatus, RecordSummary } from "$lib/types";
  import { Loader2 } from "lucide-svelte";
  import { addToast, updateToast } from "$lib/toastStore";
  import { activeView } from "$lib/store";

  let isProvisioned = $state(false);
  
  let records = $state<RecordSummary[]>([]);
  let cases = $state<CaseSummary[]>([]);
  let selectedRecord = $state<RecordSummary | null>(null);
  let selectedCaseId = $state<string | null>(null);
  let databaseStatus = $state<DatabaseStatus | null>(null);
  
  let query = $state("");
  let busy = $state<string | null>(null);
  let initializing = $state(true);
  let viewMode = $state<"grid" | "cards" | "list">("grid");
  let analysisModalOpen = $state(false);
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
      if (query.trim()) {
        const results = await invoke<any>("search", { request: { query: query.trim(), filters: null } });
        records = results.results.map((r: any) => ({
           ...r,
           source_type: r.source_type || 'official',
           entity_count: 0,
           incident_date: r.release_date,
        }));
      } else {
        records = await invoke<RecordSummary[]>("list_records", { filter: { source_type: null, local_only: null, query: null } });
      }
      
      const [nextCases, nextStatus] = await Promise.all([
        invoke<CaseSummary[]>("list_cases"),
        invoke<DatabaseStatus>("get_database_status"),
      ]);
      cases = nextCases;
      databaseStatus = nextStatus;
      if (!selectedCaseId && nextCases.length > 0) {
        selectedCaseId = nextCases[0].id;
      }
      
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
      const removed = await invoke<number>("cleanup_duplicates");
      if (removed > 0) {
        addToast({ type: "info", message: `Data integrity: Merged ${removed} duplicate records.`, duration: 3000 });
      }
      await loadInitialData();
      updateToast(toastId, { type: "info", message: "Sync complete! Downloading missing records...", duration: 3000 });
      
      $activeView = "agent";
      await invoke("download_missing_records");
      busy = null;
    } catch (e) {
      updateToast(toastId, { type: "error", message: `Sync failed: ${e}`, duration: 5000 });
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

  let systemStats = $state<any>(null);

  onMount(() => {
    const statsInterval = setInterval(async () => {
      try {
        systemStats = await invoke("get_system_stats");
      } catch (e) {
        console.warn("Failed to poll system stats", e);
      }
    }, 2000);
// Auto-detect provisioning
(async () => {
  try {
    const modelStatus = await invoke<Record<string, boolean>>("check_model_status");
    const specs = await invoke<any>("get_hardware_diagnostics");

    const required = ["bge-small", "tokenizer"];
    if (specs.recommended_tier === "Elite") {
      required.push("gemma-4b");
    } else {
      required.push("gemma-2b");
    }

    const allPresent = required.every(id => modelStatus[id]);
    if (allPresent) {
      isProvisioned = true;
    }
  } catch (e) {
    console.error("Provisioning check failed", e);
  }
})();

return () => {
  clearInterval(statsInterval);
};
});

  let hasLoaded = false;
  $effect(() => {
    if (isProvisioned && !initializing && !hasLoaded) {
        if ($activeView === 'dashboard') {
            hasLoaded = true;
            void loadInitialData();
        }
    }
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
      <div class="view-context">
        <h2 class="view-title">{(
          $activeView === 'dashboard' ? 'Evidence Archive' : 
          $activeView === 'intelligence' ? 'Neural Engine' : 
          $activeView === 'vault' ? 'Secure Vault' : 
          $activeView === 'agent' ? 'Ingestion Agent' :
          $activeView
        ).toUpperCase()}</h2>
      </div>

      <div class="header-actions">
        <GlobalActions 
          bind:query={query} 
          bind:viewMode={viewMode}
          onLoad={loadInitialData}
          onSelect={(r: RecordSummary) => (selectedRecord = r)}
          onSync={sync}
          onAnalyze={() => (analysisModalOpen = true)}
          bind:busy={busy}
        />
      </div>
    </header>

    {#if databaseStatus}
      <div class="stats-bar">
        <span class="stat">Total Records: <strong>{databaseStatus.total_count}</strong></span>
        <span class="stat">Vault Storage: <strong>{formatBytes(databaseStatus.total_size)}</strong></span>
        <span class="stat">Database: <strong>Online</strong></span>
      </div>
    {/if}

    <div class="os-body">
      <main class="os-main">
        <div class="view-container">
          {#if $activeView === 'dashboard'}
            {#if viewMode === 'grid'}
              <GridView 
                records={records} 
                selectedRecordId={selectedRecord?.id}
                onSelect={(r) => (selectedRecord = r)}
              />
            {:else if viewMode === 'cards'}
              <IntelCardsView 
                records={records} 
                selectedRecordId={selectedRecord?.id}
                onSelect={(r) => (selectedRecord = r)}
              />
            {:else if viewMode === 'list'}
              <ListView 
                records={records} 
                selectedRecordId={selectedRecord?.id}
                onSelect={(r) => (selectedRecord = r)}
              />
            {/if}
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
             <div class="view-placeholder">
               <Map records={records} onSelect={(r) => (selectedRecord = r)} />
             </div>
          {:else if $activeView === 'link-analysis'}
             <div class="view-placeholder">
               <LinkAnalysis records={records} />
             </div>
          {:else if $activeView === 'settings'}
             <Settings />
          {/if}
        </div>
      </main>

      {#if selectedRecord}
        <button 
          class="sidebar-resizer" 
          aria-label="Resize sidebar"
          onmousedown={startResizing}
          class:active={isResizing}
        ></button>
        <aside class="os-sidebar" style="width: {sidebarWidth}px; min-width: {sidebarWidth}px;">
          <IntelligenceDossier 
            record={selectedRecord} 
            cases={cases}
            selectedCaseId={selectedCaseId}
            onBack={() => (selectedRecord = null)}
            onChanged={() => loadInitialData()}
            onAnalyze={() => (analysisModalOpen = true)}
          />
        </aside>
      {/if}
    </div>

    <footer class="os-footer">
      <div class="f-section">
        <span class="f-label">Ingestion:</span>
        <span class="f-val">{databaseStatus?.local_records || 0} / {databaseStatus?.official_records || 0} Assets</span>
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
        <div class="status-orb" class:busy={busy}></div>
        <span class="f-val">{busy ? `AGENT ${busy.toUpperCase()} ACTIVE` : 'INTELLIGENCE OS STANDBY'}</span>
      </div>
    </footer>
  </div>

  <AnalysisModal bind:isOpen={analysisModalOpen} onComplete={loadInitialData} />
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
    border: none;
    padding: 0;
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
    background: rgba(255,255,255,0.05);
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
    0% { opacity: 1; transform: scale(1); }
    50% { opacity: 0.5; transform: scale(1.2); }
    100% { opacity: 1; transform: scale(1); }
  }

  .view-placeholder {
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
