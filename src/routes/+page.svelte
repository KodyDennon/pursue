<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { open } from "@tauri-apps/plugin-dialog";
  import type {
    BulkDownloadReport,
    CaseSummary,
    DatabaseStatus,
    RecordFilter,
    RecordSummary,
    SearchResults,
    SyncReport
  } from "$lib/types";
  import ArchiveViewer from "$lib/components/ArchiveViewer.svelte";
  import Map from "$lib/components/Map.svelte";

  type ViewMode = "records" | "search" | "map" | "database" | "cases";
  type SourceFilter = "all" | "official" | "manual" | "local";

  let records = $state<RecordSummary[]>([]);
  let cases = $state<CaseSummary[]>([]);
  let selectedRecord = $state<RecordSummary | null>(null);
  let selectedCaseId = $state<string | null>(null);
  let databaseStatus = $state<DatabaseStatus | null>(null);
  let viewMode = $state<ViewMode>("records");
  let sourceFilter = $state<SourceFilter>("all");
  let query = $state("");
  let loading = $state(false);
  let busy = $state<string | null>(null);
  let syncReport = $state<SyncReport | null>(null);
  let bulkReport = $state<BulkDownloadReport | null>(null);
  let activeDownloadJobId = $state<string | null>(null);
  let searchResults = $state<SearchResults | null>(null);
  let error = $state<string | null>(null);
  let diagnostics = $state<any>(null);
  let caseTitle = $state("");
  let lastExportPath = $state<string | null>(null);
  let pollHandle: ReturnType<typeof setInterval> | null = null;

  const selectedCase = $derived(cases.find((item) => item.id === selectedCaseId) ?? null);
  const localCount = $derived(records.filter((record) => record.local_path).length);
  const analyzedCount = $derived(records.filter((record) => record.analysis_status === "completed").length);
  const failedCount = $derived(records.filter((record) => record.analysis_status === "failed").length);
  const remoteCount = $derived(records.filter((record) => !record.local_path && record.document_url).length);

  function recordFilter(): RecordFilter {
    return {
      source_type: sourceFilter === "official" || sourceFilter === "manual" ? sourceFilter : null,
      local_only: sourceFilter === "local" ? true : null,
      query: query.trim() || null
    };
  }

  async function loadInitialData() {
    loading = true;
    error = null;
    try {
      const [nextRecords, nextCases, nextStatus, nextDiagnostics] = await Promise.all([
        invoke<RecordSummary[]>("list_records", { filter: recordFilter() }),
        invoke<CaseSummary[]>("list_cases"),
        invoke<DatabaseStatus>("get_database_status"),
        invoke<any>("get_hardware_diagnostics")
      ]);
      records = nextRecords;
      cases = nextCases;
      databaseStatus = nextStatus;
      diagnostics = nextDiagnostics;
      if (selectedRecord) {
        selectedRecord = nextRecords.find((record) => record.id === selectedRecord?.id) ?? selectedRecord;
      } else if (nextRecords.length > 0) {
        selectedRecord = nextRecords[0];
      }
      if (!selectedCaseId && nextCases.length > 0) {
        selectedCaseId = nextCases[0].id;
      }
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  async function refreshRecord(id: string) {
    const record = await invoke<RecordSummary | null>("get_record", { id });
    if (record) {
      selectedRecord = record;
      records = records.map((item) => (item.id === id ? record : item));
    }
  }

  async function sync() {
    busy = "sync";
    error = null;
    try {
      syncReport = await invoke<SyncReport>("sync_official_source");
      await loadInitialData();
    } catch (e) {
      error = String(e);
    } finally {
      busy = null;
    }
  }

  async function importFile() {
    busy = "import";
    error = null;
    try {
      const selected = await open({
        multiple: false,
        filters: [
          {
            name: "Evidence",
            extensions: ["pdf", "txt", "md", "csv", "json", "png", "jpg", "jpeg", "tif", "tiff", "bmp"]
          }
        ]
      });
      if (typeof selected !== "string") return;
      const record = await invoke<RecordSummary>("import_manual_file", {
        request: { path: selected, title: null, notes: null }
      });
      await loadInitialData();
      selectedRecord = record;
    } catch (e) {
      error = String(e);
    } finally {
      busy = null;
    }
  }

  async function startBulkDownload() {
    busy = "bulk-download";
    error = null;
    try {
      activeDownloadJobId = await invoke<string>("download_missing_records");
      await pollBulkDownload();
      startPolling();
    } catch (e) {
      error = String(e);
    } finally {
      busy = null;
    }
  }

  async function pollBulkDownload() {
    if (!activeDownloadJobId) return;
    try {
      bulkReport = await invoke<BulkDownloadReport>("get_bulk_download_status", { id: activeDownloadJobId });
      const status = bulkReport.job.status;
      if (!["queued", "running"].includes(status)) {
        stopPolling();
        await loadInitialData();
      }
    } catch (e) {
      error = String(e);
      stopPolling();
    }
  }

  function startPolling() {
    stopPolling();
    pollHandle = setInterval(() => {
      void pollBulkDownload();
    }, 1500);
  }

  function stopPolling() {
    if (pollHandle) {
      clearInterval(pollHandle);
      pollHandle = null;
    }
  }

  async function cancelBulkDownload() {
    if (!activeDownloadJobId) return;
    busy = "cancel-download";
    try {
      await invoke("cancel_bulk_download", { id: activeDownloadJobId });
      await pollBulkDownload();
    } catch (e) {
      error = String(e);
    } finally {
      busy = null;
    }
  }

  async function runSearch() {
    if (!query.trim()) {
      await loadInitialData();
      return;
    }
    loading = true;
    error = null;
    try {
      searchResults = await invoke<SearchResults>("search", {
        request: {
          query: query.trim(),
          filters: {
            source_type: sourceFilter === "official" || sourceFilter === "manual" ? sourceFilter : null,
            local_only: sourceFilter === "local" ? true : null
          }
        }
      });
      viewMode = "search";
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  async function selectRecordById(id: string) {
    const existing = records.find((record) => record.id === id);
    if (existing) {
      selectedRecord = existing;
      return;
    }
    await refreshRecord(id);
  }

  async function createCase() {
    if (!caseTitle.trim()) return;
    busy = "create-case";
    error = null;
    try {
      const created = await invoke<CaseSummary>("create_case", {
        request: { title: caseTitle.trim(), description: null }
      });
      caseTitle = "";
      await loadInitialData();
      selectedCaseId = created.id;
      viewMode = "cases";
    } catch (e) {
      error = String(e);
    } finally {
      busy = null;
    }
  }

  async function exportSelectedCase(format: "markdown" | "html") {
    if (!selectedCaseId) return;
    busy = `export-${format}`;
    error = null;
    try {
      const result = await invoke<{ absolute_path: string }>("export_case", {
        request: { case_id: selectedCaseId, format }
      });
      lastExportPath = result.absolute_path;
      await loadInitialData();
    } catch (e) {
      error = String(e);
    } finally {
      busy = null;
    }
  }

  function setFilter(next: SourceFilter) {
    sourceFilter = next;
    void loadInitialData();
  }

  function statusText(record: RecordSummary) {
    if (record.analysis_status === "completed") return "Indexed";
    if (record.analysis_status === "processing") return "Processing";
    if (record.analysis_status === "failed") return "Failed";
    return record.local_path ? "Local" : "Remote";
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

  function shortHash(value: string | null) {
    return value ? value.slice(0, 12) : "unverified";
  }

  onMount(() => {
    void loadInitialData();
  });

  onDestroy(() => {
    stopPolling();
  });
</script>

<svelte:head>
  <title>PURSUE Data Analyzer</title>
</svelte:head>

<div class="workstation">
  <aside class="sidebar">
    <header class="brand">
      <div>
        <span class="eyebrow">PURSUE</span>
        <h1>Evidence Workstation</h1>
      </div>
      <span class="version">0.2.1</span>
    </header>

    <nav class="nav-list" aria-label="Workspace views">
      <button class:active={viewMode === "records"} onclick={() => (viewMode = "records")}>Records</button>
      <button class:active={viewMode === "search"} onclick={() => (viewMode = "search")}>Search</button>
      <button class:active={viewMode === "map"} onclick={() => (viewMode = "map")}>Map</button>
      <button class:active={viewMode === "cases"} onclick={() => (viewMode = "cases")}>Cases</button>
      <button class:active={viewMode === "database"} onclick={() => (viewMode = "database")}>Database</button>
    </nav>

    <section class="sidebar-section">
      <h2>Filters</h2>
      <div class="segmented">
        <button class:active={sourceFilter === "all"} onclick={() => setFilter("all")}>All</button>
        <button class:active={sourceFilter === "official"} onclick={() => setFilter("official")}>Official</button>
        <button class:active={sourceFilter === "manual"} onclick={() => setFilter("manual")}>Manual</button>
        <button class:active={sourceFilter === "local"} onclick={() => setFilter("local")}>Local</button>
      </div>
    </section>

    <section class="sidebar-section">
      <h2>Library</h2>
      <dl class="metric-list">
        <div><dt>Records</dt><dd>{databaseStatus?.total_records ?? records.length}</dd></div>
        <div><dt>Local Files</dt><dd>{databaseStatus?.local_records ?? localCount}</dd></div>
        <div><dt>Remote Files</dt><dd>{remoteCount}</dd></div>
        <div><dt>Indexed</dt><dd>{databaseStatus?.analyzed_records ?? analyzedCount}</dd></div>
        <div><dt>Chunks</dt><dd>{databaseStatus?.analysis_chunks ?? 0}</dd></div>
        <div><dt>Vectors</dt><dd>{databaseStatus?.vector_chunks ?? 0}</dd></div>
      </dl>
    </section>

    <section class="sidebar-section">
      <h2>Storage</h2>
      <p class="path-line">{databaseStatus?.database_path ?? "Database not loaded"}</p>
      <p class="path-line">{databaseStatus?.library_path ?? "Library not loaded"}</p>
    </section>
  </aside>

  <main class="main-panel">
    <header class="topbar">
      <div class="search-box">
        <input
          bind:value={query}
          placeholder="Search titles, metadata, and indexed text"
          onkeydown={(event) => event.key === "Enter" && runSearch()}
        />
        <button class="primary" onclick={runSearch} disabled={loading}>Search</button>
      </div>
      <div class="top-actions">
        <button onclick={sync} disabled={busy === "sync"}>{busy === "sync" ? "Syncing" : "Sync WAR.gov"}</button>
        <button onclick={importFile} disabled={busy === "import"}>{busy === "import" ? "Importing" : "Import File"}</button>
        <button onclick={startBulkDownload} disabled={busy === "bulk-download"}>
          {busy === "bulk-download" ? "Starting" : "Download Missing"}
        </button>
      </div>
    </header>

    {#if error}
      <div class="notice error" role="alert">
        <strong>Operation failed</strong>
        <span>{error}</span>
        <button onclick={() => (error = null)}>Dismiss</button>
      </div>
    {/if}

    <section class="status-strip" aria-label="Operational status">
      <div>
        <span>Total</span>
        <strong>{records.length}</strong>
      </div>
      <div>
        <span>Local</span>
        <strong>{localCount}</strong>
      </div>
      <div>
        <span>Indexed</span>
        <strong>{analyzedCount}</strong>
      </div>
      <div>
        <span>Failed</span>
        <strong>{failedCount}</strong>
      </div>
      <div>
        <span>Artifacts</span>
        <strong>{databaseStatus?.artifact_count ?? 0}</strong>
      </div>
      <div>
        <span>Artifact Size</span>
        <strong>{formatBytes(databaseStatus?.artifact_bytes)}</strong>
      </div>
      <div>
        <span>Hardware</span>
        <strong>{diagnostics?.tier ?? "Detecting"}</strong>
      </div>
    </section>

    {#if syncReport}
      <div class="notice">
        <strong>Last sync</strong>
        <span>{syncReport.record_count} records, {syncReport.added} added, {syncReport.changed} changed, {syncReport.removed} removed</span>
      </div>
    {/if}

    {#if bulkReport}
      <section class="download-panel">
        <div>
          <h2>Bulk Download</h2>
          <p>{bulkReport.job.status}: {bulkReport.job.completed} completed, {bulkReport.job.failed} failed, {bulkReport.job.skipped} skipped</p>
        </div>
        <progress max={Math.max(bulkReport.job.queued, bulkReport.job.completed + bulkReport.job.failed, 1)} value={bulkReport.job.completed + bulkReport.job.failed}></progress>
        <button onclick={cancelBulkDownload} disabled={!["queued", "running"].includes(bulkReport.job.status) || busy === "cancel-download"}>Cancel</button>
      </section>
    {/if}

    <div class="workspace-grid">
      <section class="workspace">
        {#if viewMode === "records"}
          <header class="section-head">
            <div>
              <h2>Evidence Records</h2>
              <p>All synced and manually imported records. Select a row to work the file.</p>
            </div>
            <button onclick={loadInitialData} disabled={loading}>{loading ? "Loading" : "Refresh"}</button>
          </header>

          <div class="table-wrap">
            <table>
              <thead>
                <tr>
                  <th>Title</th>
                  <th>Agency</th>
                  <th>Release</th>
                  <th>Source</th>
                  <th>File</th>
                  <th>Analysis</th>
                  <th>Entities</th>
                </tr>
              </thead>
              <tbody>
                {#each records as record}
                  <tr class:selected={selectedRecord?.id === record.id} onclick={() => (selectedRecord = record)}>
                    <td>
                      <strong>{record.title}</strong>
                      <small>{record.incident_location || record.stable_key || "No location/key"}</small>
                    </td>
                    <td>{record.agency || "Unknown"}</td>
                    <td>{record.release_date || "Undated"}</td>
                    <td><span class="badge">{record.source_type}</span></td>
                    <td>{record.local_path ? formatBytes(record.artifact_size) : "remote"}</td>
                    <td><span class="status {record.analysis_status || 'pending'}">{statusText(record)}</span></td>
                    <td>{record.entity_count}</td>
                  </tr>
                {/each}
              </tbody>
            </table>
          </div>
        {:else if viewMode === "search"}
          <header class="section-head">
            <div>
              <h2>Semantic and Text Search</h2>
              <p>Search returns indexed chunks after analysis, with keyword fallback when vector search is unavailable.</p>
            </div>
            <button onclick={runSearch} disabled={loading}>{loading ? "Searching" : "Run Search"}</button>
          </header>
          {#if searchResults}
            <div class="search-results">
              {#each searchResults.results as result}
                <button class="result-row" onclick={() => selectRecordById(result.id)}>
                  <span>{result.agency || "Unknown"} · {(Math.max(0, 1 - result.distance) * 100).toFixed(1)}%</span>
                  <strong>{result.title}</strong>
                  <p>{result.excerpt}</p>
                </button>
              {/each}
              {#if searchResults.results.length === 0}
                <div class="empty-state">No indexed matches. Download and analyze records, then search again.</div>
              {/if}
            </div>
          {:else}
            <div class="empty-state">Enter a query and run search.</div>
          {/if}
        {:else if viewMode === "map"}
          <header class="section-head">
            <div>
              <h2>Incident Map</h2>
              <p>Records with recognizable incident locations are plotted here.</p>
            </div>
          </header>
          <div class="map-frame">
            <Map {records} onSelect={(record) => (selectedRecord = record)} />
          </div>
        {:else if viewMode === "cases"}
          <header class="section-head">
            <div>
              <h2>Cases and Exports</h2>
              <p>Create working cases, add selected records, and export Markdown or HTML dossiers.</p>
            </div>
          </header>

          <div class="case-workspace">
            <form class="inline-form" onsubmit={(event) => { event.preventDefault(); void createCase(); }}>
              <input bind:value={caseTitle} placeholder="New case title" />
              <button class="primary" disabled={!caseTitle.trim() || busy === "create-case"}>Create Case</button>
            </form>

            <div class="case-list">
              {#each cases as item}
                <button class:selected={selectedCaseId === item.id} onclick={() => (selectedCaseId = item.id)}>
                  <strong>{item.title}</strong>
                  <span>{item.record_count} records · {item.note_count} notes</span>
                </button>
              {/each}
              {#if cases.length === 0}
                <div class="empty-state">No cases yet.</div>
              {/if}
            </div>

            <div class="case-actions">
              <span>{selectedCase ? selectedCase.title : "No case selected"}</span>
              <button onclick={() => exportSelectedCase("markdown")} disabled={!selectedCaseId || busy === "export-markdown"}>Export MD</button>
              <button onclick={() => exportSelectedCase("html")} disabled={!selectedCaseId || busy === "export-html"}>Export HTML</button>
            </div>
            {#if lastExportPath}
              <p class="path-line">Last export: {lastExportPath}</p>
            {/if}
          </div>
        {:else if viewMode === "database"}
          <header class="section-head">
            <div>
              <h2>Database and Indexes</h2>
              <p>Local SQLite, artifact library, snapshots, exports, chunks, vectors, and entity counts.</p>
            </div>
            <button onclick={loadInitialData}>Refresh</button>
          </header>

          <div class="database-grid">
            <dl>
              <div><dt>Database</dt><dd>{databaseStatus?.database_path}</dd></div>
              <div><dt>Library</dt><dd>{databaseStatus?.library_path}</dd></div>
              <div><dt>Snapshots</dt><dd>{databaseStatus?.snapshots_path}</dd></div>
              <div><dt>Exports</dt><dd>{databaseStatus?.exports_path}</dd></div>
              <div><dt>Latest Snapshot</dt><dd>{databaseStatus?.latest_snapshot_at || "No sync yet"}</dd></div>
              <div><dt>Source URL</dt><dd>{databaseStatus?.latest_snapshot_url || "No upstream recorded"}</dd></div>
            </dl>
            <dl>
              <div><dt>Official</dt><dd>{databaseStatus?.official_records ?? 0}</dd></div>
              <div><dt>Manual</dt><dd>{databaseStatus?.manual_records ?? 0}</dd></div>
              <div><dt>Downloadable</dt><dd>{databaseStatus?.downloadable_records ?? 0}</dd></div>
              <div><dt>Artifacts</dt><dd>{databaseStatus?.artifact_count ?? 0}</dd></div>
              <div><dt>Analysis Chunks</dt><dd>{databaseStatus?.analysis_chunks ?? 0}</dd></div>
              <div><dt>Vector Rows</dt><dd>{databaseStatus?.vector_chunks ?? 0}</dd></div>
              <div><dt>Entities</dt><dd>{databaseStatus?.entity_count ?? 0}</dd></div>
            </dl>
          </div>
        {/if}
      </section>

      <aside class="detail-panel">
        {#if selectedRecord}
          <ArchiveViewer
            record={selectedRecord}
            cases={cases}
            selectedCaseId={selectedCaseId}
            onBack={() => (selectedRecord = null)}
            onChanged={async () => {
              await refreshRecord(selectedRecord?.id || "");
              await loadInitialData();
            }}
          />
        {:else}
          <div class="empty-state">Select a record to see actions, metadata, analysis, and case tools.</div>
        {/if}
      </aside>
    </div>
  </main>
</div>
