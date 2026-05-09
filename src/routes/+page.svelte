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
  import EmptyState from "$lib/components/dashboard/EmptyState.svelte";
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

  async function loadInitialData() {
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
    } catch (e) {
      addToast({ type: "error", message: `Failed to load data: ${e}`, duration: 5000 });
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
  <div class="os-container">
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
          {/if}
        {/if}
      </main>

      {#if selectedRecord}
        <aside class="os-sidebar">
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
    width: 540px;
    min-width: 540px;
    height: 100%;
  }

  .map-view, .link-view {
    height: 100%;
    width: 100%;
  }
</style>
