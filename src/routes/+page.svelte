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
  let viewMode = $state<"records" | "map" | "search">("records");
  let sourceFilter = $state<"all" | "official" | "manual" | "local">("all");
  let query = $state("");
  let loading = $state(false);
  let syncing = $state(false);
  let importing = $state(false);
  let syncReport = $state<SyncReport | null>(null);
  let bulkReport = $state<BulkDownloadReport | null>(null);
  let searchResults = $state<SearchResults | null>(null);
  let newCaseTitle = $state("");
  let notice = $state<string | null>(null);
  let error = $state<string | null>(null);
  let pollTimer: ReturnType<typeof setInterval> | null = null;

  const selectedCase = $derived(cases.find((item) => item.id === selectedCaseId) ?? null);
  const localCount = $derived(records.filter((record) => record.local_path).length);
  const analyzedCount = $derived(records.filter((record) => record.analysis_status === "completed").length);
  const removedCount = $derived(records.filter((record) => record.removed_from_source_at).length);

  function recordFilter(): RecordFilter {
    return {
      source_type: sourceFilter === "official" || sourceFilter === "manual" ? sourceFilter : null,
      local_only: sourceFilter === "local" ? true : null,
      query: query.trim() || null
    };
  }

  async function loadRecords() {
    loading = true;
    error = null;
    try {
      records = await invoke<RecordSummary[]>("list_records", { filter: recordFilter() });
    } catch (caught) {
      error = String(caught);
    } finally {
      loading = false;
    }
  }

  async function loadCases() {
    try {
      cases = await invoke<CaseSummary[]>("list_cases");
      if (!selectedCaseId && cases.length > 0) selectedCaseId = cases[0].id;
    } catch (caught) {
      error = String(caught);
    }
  }

  async function syncOfficialSource() {
    syncing = true;
    error = null;
    try {
      syncReport = await invoke<SyncReport>("sync_official_source");
      notice = `Sync complete: ${syncReport.added} added, ${syncReport.changed} changed, ${syncReport.removed} removed`;
      await loadRecords();
    } catch (caught) {
      error = String(caught);
    } finally {
      syncing = false;
    }
  }

  async function downloadRecord(id: string) {
    error = null;
    try {
      await invoke("download_record", { id });
      await loadRecords();
    } catch (caught) {
      error = String(caught);
    }
  }

  async function downloadMissingRecords() {
    error = null;
    try {
      const id = await invoke<string>("download_missing_records");
      await pollBulkJob(id);
      if (pollTimer) clearInterval(pollTimer);
      pollTimer = setInterval(() => void pollBulkJob(id), 1500);
    } catch (caught) {
      error = String(caught);
    }
  }

  async function pollBulkJob(id: string) {
    bulkReport = await invoke<BulkDownloadReport>("get_bulk_download_status", { id });
    const status = bulkReport.job.status;
    if (status === "completed" || status === "completed_with_errors" || status === "failed" || status === "cancelled") {
      if (pollTimer) clearInterval(pollTimer);
      pollTimer = null;
      await loadRecords();
    }
  }

  async function cancelBulkDownload() {
    if (!bulkReport) return;
    await invoke("cancel_bulk_download", { id: bulkReport.job.id });
    await pollBulkJob(bulkReport.job.id);
  }

  async function importManualFile() {
    importing = true;
    error = null;
    try {
      const path = await open({
        multiple: false,
        filters: [
          { name: "Evidence", extensions: ["pdf", "png", "jpg", "jpeg", "tif", "tiff", "txt", "csv", "json", "mp4", "mov"] }
        ]
      });
      if (typeof path !== "string") return;
      const imported = await invoke<RecordSummary>("import_manual_file", {
        request: { path, title: null, notes: null }
      });
      selectedRecord = imported;
      await loadRecords();
    } catch (caught) {
      error = String(caught);
    } finally {
      importing = false;
    }
  }

  async function runSearch() {
    if (!query.trim()) {
      searchResults = null;
      viewMode = "records";
      await loadRecords();
      return;
    }
    error = null;
    try {
      searchResults = await invoke<SearchResults>("search", {
        request: { query, filters: { local_only: sourceFilter === "local" ? true : null } }
      });
      viewMode = "search";
    } catch (caught) {
      error = String(caught);
    }
  }

  async function createCase() {
    const title = newCaseTitle.trim();
    if (!title) return;
    error = null;
    try {
      const created = await invoke<CaseSummary>("create_case", {
        request: { title, description: null }
      });
      newCaseTitle = "";
      selectedCaseId = created.id;
      await loadCases();
    } catch (caught) {
      error = String(caught);
    }
  }

  async function handleRecordChanged(record: RecordSummary) {
    selectedRecord = record;
    await loadRecords();
    await loadCases();
  }

  onMount(() => {
    void loadRecords();
    void loadCases();
  });

  onDestroy(() => {
    if (pollTimer) clearInterval(pollTimer);
  });
</script>

<svelte:head>
  <title>PURSUE Data Analyzer</title>
</svelte:head>

<div class="app-shell">
  <header class="topbar">
    <div class="brand">
      <div class="brand-mark">P</div>
      <div>
        <h1>PURSUE Data Analyzer</h1>
        <p>WAR.gov source sync · local evidence · cases · dossiers</p>
      </div>
    </div>
    <div class="top-actions">
      <button class="secondary" onclick={importManualFile} disabled={importing}>{importing ? "Importing" : "Import"}</button>
      <button class="secondary" onclick={downloadMissingRecords}>Download Missing</button>
      <button class="primary" onclick={syncOfficialSource} disabled={syncing}>{syncing ? "Syncing" : "Sync WAR.gov"}</button>
    </div>
  </header>

  <main class="workspace">
    <aside class="sidebar">
      <section class="metric-grid">
        <div><strong>{records.length}</strong><span>records</span></div>
        <div><strong>{localCount}</strong><span>local</span></div>
        <div><strong>{analyzedCount}</strong><span>indexed</span></div>
        <div><strong>{removedCount}</strong><span>removed</span></div>
      </section>

      <section class="panel">
        <h2>Sources</h2>
        <button class:active={sourceFilter === "all"} onclick={() => { sourceFilter = "all"; void loadRecords(); }}>All records</button>
        <button class:active={sourceFilter === "official"} onclick={() => { sourceFilter = "official"; void loadRecords(); }}>Official</button>
        <button class:active={sourceFilter === "manual"} onclick={() => { sourceFilter = "manual"; void loadRecords(); }}>Manual</button>
        <button class:active={sourceFilter === "local"} onclick={() => { sourceFilter = "local"; void loadRecords(); }}>Local files</button>
      </section>

      <section class="panel">
        <h2>Cases</h2>
        <div class="case-create">
          <input bind:value={newCaseTitle} placeholder="Case title" onkeydown={(event) => event.key === "Enter" && void createCase()} />
          <button onclick={createCase}>+</button>
        </div>
        <div class="case-list">
          {#each cases as item}
            <button class:active={selectedCaseId === item.id} onclick={() => selectedCaseId = item.id}>
              <span>{item.title}</span>
              <small>{item.record_count} / {item.note_count}</small>
            </button>
          {/each}
        </div>
      </section>

      {#if syncReport}
        <section class="panel compact">
          <h2>Latest Sync</h2>
          <p>{syncReport.fetched_at}</p>
          <div class="diff-row">
            <span>+{syncReport.added}</span>
            <span>~{syncReport.changed}</span>
            <span>-{syncReport.removed}</span>
          </div>
        </section>
      {/if}

      {#if bulkReport}
        <section class="panel compact">
          <h2>Download Queue</h2>
          <p>{bulkReport.job.status}</p>
          <progress max={bulkReport.job.queued || 1} value={bulkReport.job.completed + bulkReport.job.failed}></progress>
          <div class="diff-row">
            <span>{bulkReport.job.completed} done</span>
            <span>{bulkReport.job.failed} failed</span>
            <span>{bulkReport.job.skipped} skip</span>
          </div>
          {#if bulkReport.job.status === "running"}
            <button class="danger" onclick={cancelBulkDownload}>Cancel</button>
          {/if}
        </section>
      {/if}
    </aside>

    <section class="content">
      {#if selectedRecord}
        <ArchiveViewer
          record={selectedRecord}
          cases={cases}
          selectedCaseId={selectedCaseId}
          onBack={() => selectedRecord = null}
          onChanged={handleRecordChanged}
        />
      {:else}
        <div class="toolbar">
          <div class="searchbox">
            <input
              bind:value={query}
              placeholder="Search metadata, OCR text, entities"
              onkeydown={(event) => event.key === "Enter" && void runSearch()}
            />
            <button onclick={runSearch}>Search</button>
          </div>
          <div class="mode-switch">
            <button class:active={viewMode === "records"} onclick={() => viewMode = "records"}>Table</button>
            <button class:active={viewMode === "map"} onclick={() => viewMode = "map"}>Map</button>
            <button class:active={viewMode === "search"} onclick={runSearch}>Hits</button>
          </div>
        </div>

        {#if error}
          <div class="banner error">{error}</div>
        {:else if notice}
          <div class="banner">{notice}</div>
        {/if}

        {#if loading}
          <div class="empty">Loading local index</div>
        {:else if viewMode === "map"}
          <div class="map-frame"><Map {records} /></div>
        {:else if viewMode === "search" && searchResults}
          <div class="results">
            {#each searchResults.results as result}
              <button class="result" onclick={() => selectedRecord = result.record}>
                <div>
                  <strong>{result.record.title}</strong>
                  <p>{result.excerpt || result.record.summary || "No excerpt available"}</p>
                </div>
                <span>{result.score.toFixed(2)}</span>
              </button>
            {/each}
            {#if searchResults.results.length === 0}
              <div class="empty">No indexed matches</div>
            {/if}
          </div>
        {:else if records.length === 0}
          <div class="empty">
            <button class="primary" onclick={syncOfficialSource}>Sync WAR.gov</button>
          </div>
        {:else}
          <div class="table-wrap">
            <table>
              <thead>
                <tr>
                  <th>Evidence</th>
                  <th>Agency</th>
                  <th>Release</th>
                  <th>Incident</th>
                  <th>Status</th>
                  <th></th>
                </tr>
              </thead>
              <tbody>
                {#each records as record}
                  <tr class:removed={Boolean(record.removed_from_source_at)} onclick={() => selectedRecord = record}>
                    <td>
                      <strong>{record.title}</strong>
                      <small>{record.summary || record.document_url || "No source summary"}</small>
                    </td>
                    <td>{record.agency || "UNKNOWN"}</td>
                    <td>{record.release_date || "N/A"}</td>
                    <td>{record.incident_date || "N/A"}</td>
                    <td>
                      <span class="pill">{record.local_path ? "local" : "remote"}</span>
                      {#if record.analysis_status}<span class="pill">{record.analysis_status}</span>{/if}
                      {#if record.entity_count}<span class="pill">{record.entity_count} entities</span>{/if}
                    </td>
                    <td>
                      {#if record.local_path}
                        <button class="row-action" onclick={(event) => { event.stopPropagation(); selectedRecord = record; }}>Open</button>
                      {:else if record.document_url}
                        <button class="row-action" onclick={(event) => { event.stopPropagation(); void downloadRecord(record.id); }}>Get</button>
                      {/if}
                    </td>
                  </tr>
                {/each}
              </tbody>
            </table>
          </div>
        {/if}
      {/if}
    </section>
  </main>
</div>

<style>
  :global(body) {
    margin: 0;
    overflow: hidden;
    background: #101114;
    color: #f4f1e8;
    font-family: ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
  }

  .app-shell {
    height: 100vh;
    display: flex;
    flex-direction: column;
    background:
      linear-gradient(90deg, rgba(255,255,255,0.035) 1px, transparent 1px) 0 0 / 44px 44px,
      linear-gradient(0deg, rgba(255,255,255,0.025) 1px, transparent 1px) 0 0 / 44px 44px,
      #101114;
  }

  .topbar {
    height: 72px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 22px;
    border-bottom: 1px solid #303238;
    background: rgba(16, 17, 20, 0.92);
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 14px;
  }

  .brand-mark {
    width: 42px;
    height: 42px;
    display: grid;
    place-items: center;
    background: #e7c46b;
    color: #141414;
    font-weight: 900;
    border-radius: 4px;
  }

  h1, h2, p {
    margin: 0;
  }

  h1 {
    font-size: 18px;
    letter-spacing: 0;
  }

  .brand p {
    color: #9da3ad;
    font-size: 12px;
    margin-top: 2px;
  }

  .top-actions,
  .mode-switch,
  .diff-row {
    display: flex;
    gap: 8px;
    align-items: center;
  }

  button,
  input {
    font: inherit;
  }

  button {
    border: 1px solid #3a3d45;
    background: #1b1d22;
    color: #f4f1e8;
    border-radius: 6px;
    padding: 8px 11px;
    cursor: pointer;
  }

  button:hover {
    border-color: #e7c46b;
  }

  button:disabled {
    cursor: wait;
    opacity: 0.6;
  }

  button.primary {
    background: #e7c46b;
    color: #121212;
    border-color: #e7c46b;
    font-weight: 800;
  }

  button.active,
  .panel button.active {
    background: #27364c;
    border-color: #5a7db2;
  }

  button.danger {
    border-color: #9f3a3a;
    color: #ffb4b4;
    width: 100%;
  }

  .workspace {
    min-height: 0;
    flex: 1;
    display: grid;
    grid-template-columns: 286px minmax(0, 1fr);
  }

  .sidebar {
    min-height: 0;
    overflow-y: auto;
    border-right: 1px solid #303238;
    background: rgba(20, 22, 27, 0.84);
    padding: 14px;
  }

  .metric-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
    margin-bottom: 14px;
  }

  .metric-grid div,
  .panel {
    border: 1px solid #303238;
    background: rgba(9, 10, 12, 0.46);
    border-radius: 8px;
  }

  .metric-grid div {
    padding: 12px;
  }

  .metric-grid strong {
    display: block;
    font-size: 24px;
  }

  .metric-grid span,
  small {
    color: #9da3ad;
    font-size: 11px;
  }

  .panel {
    padding: 12px;
    margin-bottom: 14px;
  }

  .panel h2 {
    color: #e7c46b;
    font-size: 11px;
    text-transform: uppercase;
    margin-bottom: 10px;
  }

  .panel > button,
  .case-list button {
    display: flex;
    width: 100%;
    justify-content: space-between;
    margin-top: 6px;
  }

  .compact p {
    color: #9da3ad;
    font-size: 12px;
    word-break: break-word;
  }

  progress {
    width: 100%;
    margin: 10px 0;
  }

  .case-create {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 38px;
    gap: 8px;
  }

  input {
    min-width: 0;
    border: 1px solid #303238;
    background: #101114;
    color: #f4f1e8;
    border-radius: 6px;
    padding: 9px 10px;
    outline: none;
  }

  input:focus {
    border-color: #e7c46b;
  }

  .content {
    min-width: 0;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }

  .toolbar {
    display: flex;
    gap: 12px;
    align-items: center;
    padding: 14px;
    border-bottom: 1px solid #303238;
    background: rgba(16, 17, 20, 0.78);
  }

  .searchbox {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 8px;
    flex: 1;
  }

  .banner {
    margin: 12px 14px 0;
    border: 1px solid #4d6f8f;
    color: #d5e9ff;
    background: rgba(43, 81, 115, 0.28);
    border-radius: 8px;
    padding: 10px 12px;
  }

  .banner.error {
    border-color: #934343;
    color: #ffd4d4;
    background: rgba(105, 35, 35, 0.32);
  }

  .empty {
    flex: 1;
    display: grid;
    place-items: center;
    color: #9da3ad;
  }

  .map-frame {
    min-height: 0;
    flex: 1;
  }

  .table-wrap,
  .results {
    min-height: 0;
    flex: 1;
    overflow: auto;
  }

  table {
    width: 100%;
    border-collapse: collapse;
  }

  th {
    position: sticky;
    top: 0;
    background: #17191e;
    color: #9da3ad;
    font-size: 11px;
    text-align: left;
    text-transform: uppercase;
    z-index: 1;
  }

  th,
  td {
    border-bottom: 1px solid #272a31;
    padding: 12px 14px;
    vertical-align: top;
  }

  tr {
    cursor: pointer;
  }

  tr:hover {
    background: rgba(231, 196, 107, 0.06);
  }

  tr.removed {
    opacity: 0.58;
  }

  td strong {
    display: block;
    font-size: 14px;
  }

  td small {
    display: block;
    max-width: 620px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    margin-top: 3px;
  }

  .pill {
    display: inline-block;
    border: 1px solid #3a3d45;
    border-radius: 999px;
    color: #bcc4cf;
    font-size: 11px;
    padding: 2px 7px;
    margin-right: 4px;
    margin-bottom: 4px;
  }

  .row-action {
    min-width: 54px;
  }

  .result {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 16px;
    border-bottom: 1px solid #272a31;
    padding: 16px;
    cursor: pointer;
  }

  .result:hover {
    background: rgba(231, 196, 107, 0.06);
  }

  .result p {
    color: #b8bec8;
    margin-top: 6px;
  }
</style>
