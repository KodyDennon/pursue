<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { open } from "@tauri-apps/plugin-dialog";
  import type {
    BulkDownloadReport,
    CaseSummary,
    RecordFilter,
    RecordSummary,
    SearchResults,
    SyncReport
  } from "$lib/types";
  import ArchiveViewer from "$lib/components/ArchiveViewer.svelte";
  import Map from "$lib/components/Map.svelte";

  let records = $state<RecordSummary[]>([]);
  let cases = $state<CaseSummary[]>([]);
  let selectedRecord = $state<RecordSummary | null>(null);
  let selectedCaseId = $state<string | null>(null);
  let viewMode = $state<"dossiers" | "map" | "search" | "dashboard">("dashboard");
  let sourceFilter = $state<"all" | "official" | "manual" | "local">("all");
  let query = $state("");
  let loading = $state(false);
  let syncing = $state(false);
  let syncReport = $state<SyncReport | null>(null);
  let bulkReport = $state<BulkDownloadReport | null>(null);
  let searchResults = $state<SearchResults | null>(null);
  let error = $state<string | null>(null);
  let diagnostics = $state<any>(null);

  const localCount = $derived(records.filter((r) => r.local_path).length);
  const analyzedCount = $derived(records.filter((r) => r.analysis_status === "completed").length);

  async function loadInitialData() {
    loading = true;
    try {
      [records, cases, diagnostics] = await Promise.all([
        invoke<RecordSummary[]>("list_records", { filter: recordFilter() }),
        invoke<CaseSummary[]>("list_cases"),
        invoke<any>("get_hardware_diagnostics")
      ]);
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  function recordFilter(): RecordFilter {
    return {
      source_type: sourceFilter === "official" || sourceFilter === "manual" ? sourceFilter : null,
      local_only: sourceFilter === "local" ? true : null,
      query: query.trim() || null
    };
  }

  async function sync() {
    syncing = true;
    try {
      syncReport = await invoke<SyncReport>("sync_official_source");
      await loadInitialData();
    } catch (e) {
      error = String(e);
    } finally {
      syncing = false;
    }
  }

  async function runSearch() {
    if (!query.trim()) return;
    loading = true;
    try {
      searchResults = await invoke<SearchResults>("search", { request: { query: query.trim() } });
      viewMode = "search";
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  onMount(() => {
    void loadInitialData();
  });
</script>

<div class="terminal-shell">
  <nav class="side-nav glass">
    <div class="logo">
      <div class="pulse"></div>
      <span>PURSUE</span>
    </div>
    
    <div class="nav-links">
      <button class:active={viewMode === 'dashboard'} onclick={() => viewMode = 'dashboard'}>
        <span class="icon">⊞</span> Dashboard
      </button>
      <button class:active={viewMode === 'dossiers'} onclick={() => viewMode = 'dossiers'}>
        <span class="icon">📁</span> Dossiers
      </button>
      <button class:active={viewMode === 'map'} onclick={() => viewMode = 'map'}>
        <span class="icon">🌍</span> Tactical Map
      </button>
    </div>

    <div class="telemetry">
      <div class="label">Hardware Intelligence</div>
      {#if diagnostics}
        <div class="stat">
          <span>Tier</span>
          <span class="value">{diagnostics.tier}</span>
        </div>
        <div class="stat">
          <span>GPU</span>
          <span class="value">{diagnostics.gpu_info.name}</span>
        </div>
      {/if}
    </div>
  </nav>

  <main class="content-area">
    <header class="content-header glass">
      <div class="search-bar">
        <input bind:value={query} placeholder="Search semantic intelligence..." onkeydown={(e) => e.key === 'Enter' && runSearch()} />
      </div>
      <div class="header-actions">
        <button class="secondary" onclick={sync} disabled={syncing}>
          {syncing ? 'Syncing...' : 'Sync Source'}
        </button>
      </div>
    </header>

    <div class="scroll-container">
      {#if selectedRecord}
        <ArchiveViewer
          record={selectedRecord}
          cases={cases}
          selectedCaseId={selectedCaseId}
          onBack={() => selectedRecord = null}
          onChanged={() => loadInitialData()}
        />
      {:else if viewMode === 'dashboard'}
        <div class="dashboard-grid">
          <div class="stats-row">
            <div class="card glass">
              <span class="card-label">Total Evidence</span>
              <div class="val">{records.length}</div>
            </div>
            <div class="card glass">
              <span class="card-label">Intelligence Indexed</span>
              <div class="val">{analyzedCount}</div>
            </div>
            <div class="card glass">
              <span class="card-label">Local Cache</span>
              <div class="val">{localCount}</div>
            </div>
          </div>

          <div class="recent-intel">
            <h2>Recent Intelligence Dossiers</h2>
            <div class="intel-list">
              {#each records.slice(0, 10) as record}
                <button class="dossier-card glass" onclick={() => selectedRecord = record}>
                  <div class="d-header">
                    <strong>{record.title}</strong>
                    <span class="pill">{record.agency}</span>
                  </div>
                  <p>{record.summary || 'No summary available'}</p>
                </button>
              {/each}
            </div>
          </div>
        </div>
      {:else if viewMode === 'map'}
        <div class="map-container glass">
          <Map {records} onSelect={(record) => { selectedRecord = record; viewMode = 'dashboard'; }} />
        </div>
      {:else if viewMode === 'dossiers'}
        <div class="table-container glass">
          <table>
            <thead>
              <tr>
                <th>Record</th>
                <th>Status</th>
                <th>Tier</th>
              </tr>
            </thead>
            <tbody>
              {#each records as record}
                <tr onclick={() => selectedRecord = record}>
                  <td>{record.title}</td>
                  <td><span class="pill {record.analysis_status}">{record.analysis_status || 'Pending'}</span></td>
                  <td>{record.local_path ? 'Local' : 'Remote'}</td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      {:else if viewMode === 'search'}
        <div class="search-results-panel">
          <header class="results-header">
            <h2>Intelligence Search Results</h2>
            <p>Found {searchResults?.total || 0} relevant matches for "{searchResults?.query}"</p>
          </header>
          <div class="results-grid">
            {#each searchResults?.results || [] as result}
              <button class="result-item glass" onclick={async () => {
                loading = true;
                try {
                  const fullRecord = await invoke<RecordSummary[]>("list_records", { filter: { query: result.id } });
                  if (fullRecord && fullRecord[0]) {
                    selectedRecord = fullRecord[0];
                  }
                } finally {
                  loading = false;
                }
              }}>
                <div class="r-meta">
                  <span class="r-agency">{result.agency}</span>
                  <span class="r-score">{(100 - (result.distance * 100)).toFixed(1)}% Match</span>
                </div>
                <h3>{result.title}</h3>
                <p class="excerpt">...{@html result.excerpt}...</p>
              </button>
            {/each}
          </div>
        </div>
      {/if}
    </div>
  </main>
</div>

<style>
  .terminal-shell {
    display: grid;
    grid-template-columns: 260px 1fr;
    height: 100vh;
    background: radial-gradient(circle at 50% 50%, #1a1c22 0%, #0a0b0d 100%);
  }

  .side-nav {
    padding: 24px;
    display: flex;
    flex-direction: column;
    gap: 32px;
    border-right: 1px solid var(--border-dim);
  }

  .logo {
    display: flex;
    align-items: center;
    gap: 12px;
    font-size: 20px;
    font-weight: 800;
    letter-spacing: 0.2em;
    color: var(--accent-gold);
  }

  .pulse {
    width: 12px;
    height: 12px;
    background: var(--accent-gold);
    border-radius: 50%;
    box-shadow: 0 0 10px var(--accent-gold);
    animation: pulse 2s infinite;
  }

  @keyframes pulse {
    0% { transform: scale(1); opacity: 1; }
    50% { transform: scale(1.5); opacity: 0.5; }
    100% { transform: scale(1); opacity: 1; }
  }

  .nav-links {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .nav-links button {
    justify-content: flex-start;
    background: transparent;
    border: none;
    text-align: left;
    padding: 12px;
    font-size: 15px;
    color: var(--text-secondary);
  }

  .nav-links button.active {
    color: var(--accent-gold);
    background: var(--accent-gold-dim);
    border-left: 2px solid var(--accent-gold);
  }

  .telemetry {
    margin-top: auto;
    padding: 16px;
    background: rgba(0,0,0,0.2);
    border-radius: 12px;
  }

  .telemetry .label {
    font-size: 10px;
    text-transform: uppercase;
    color: var(--text-secondary);
    margin-bottom: 12px;
  }

  .stat {
    display: flex;
    justify-content: space-between;
    font-size: 12px;
    margin-bottom: 6px;
  }

  .stat .value {
    color: var(--accent-gold);
  }

  .content-area {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .content-header {
    height: 72px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 24px;
    border-bottom: 1px solid var(--border-dim);
    z-index: 10;
  }

  .search-bar {
    flex: 1;
    max-width: 600px;
  }

  .search-bar input {
    width: 100%;
    background: rgba(0,0,0,0.3);
    border: 1px solid var(--border-dim);
    padding: 12px 16px;
    border-radius: 8px;
    color: white;
    outline: none;
  }

  .search-bar input:focus {
    border-color: var(--accent-gold);
  }

  .scroll-container {
    flex: 1;
    overflow-y: auto;
    padding: 24px;
  }

  .dashboard-grid {
    display: flex;
    flex-direction: column;
    gap: 32px;
  }

  .stats-row {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 24px;
  }

  .card {
    padding: 24px;
    border-radius: 16px;
  }

  .card-label {
    font-size: 12px;
    color: var(--text-secondary);
    text-transform: uppercase;
    letter-spacing: 0.1em;
  }

  .card .val {
    font-size: 36px;
    font-weight: 800;
    margin-top: 8px;
    color: var(--accent-gold);
  }

  .recent-intel h2 {
    margin-bottom: 24px;
    font-size: 18px;
    color: var(--text-secondary);
  }

  .intel-list {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
    gap: 16px;
  }

  .dossier-card {
    display: block;
    width: 100%;
    text-align: left;
    background: transparent;
    border: 1px solid var(--border-dim);
    border-radius: 16px;
    padding: 20px;
    cursor: pointer;
    transition: transform 0.2s, border-color 0.2s;
  }

  .dossier-card:hover {
    transform: scale(1.02);
    border-color: var(--accent-gold);
  }

  .d-header {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    margin-bottom: 12px;
  }

  .pill {
    font-size: 10px;
    padding: 2px 8px;
    border-radius: 99px;
    background: var(--bg-secondary);
    border: 1px solid var(--border-dim);
  }

  .pill.completed { background: #1a4d2e; color: #a8d9bb; }
  .pill.processing { background: #1a364d; color: #a8cde7; }

  .map-container {
    height: 700px;
    border-radius: 20px;
    overflow: hidden;
  }

  .table-container {
    border-radius: 16px;
    overflow: hidden;
  }

  table {
    width: 100%;
    border-collapse: collapse;
  }

  th, td {
    padding: 16px;
    text-align: left;
    border-bottom: 1px solid var(--border-dim);
  }

  th {
    font-size: 12px;
    text-transform: uppercase;
    color: var(--text-secondary);
  }

  tr:hover {
    background: rgba(255,255,255,0.03);
    cursor: pointer;
  }

  .search-results-panel {
    display: flex;
    flex-direction: column;
    gap: 32px;
  }

  .results-header h2 {
    margin: 0 0 8px;
    font-size: 24px;
    color: var(--accent-gold);
  }

  .results-header p {
    margin: 0;
    color: var(--text-secondary);
  }

  .results-grid {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .result-item {
    display: block;
    width: 100%;
    text-align: left;
    background: rgba(255,255,255,0.02);
    border: 1px solid var(--border-dim);
    border-radius: 12px;
    padding: 24px;
    cursor: pointer;
    transition: all 0.2s;
  }

  .result-item:hover {
    border-color: var(--accent-gold);
    background: rgba(255,255,255,0.04);
    transform: translateX(4px);
  }

  .r-meta {
    display: flex;
    justify-content: space-between;
    margin-bottom: 12px;
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.1em;
  }

  .r-agency {
    color: var(--accent-gold);
  }

  .r-score {
    color: var(--text-secondary);
  }

  .result-item h3 {
    margin: 0 0 12px;
    font-size: 18px;
  }

  .excerpt {
    font-size: 14px;
    color: var(--text-secondary);
    line-height: 1.6;
    margin: 0;
  }
</style>
