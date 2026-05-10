<script lang="ts">
  import { convertFileSrc, invoke } from "@tauri-apps/api/core";
  import { openPath, openUrl } from "@tauri-apps/plugin-opener";
  import type { AnalysisReport, CaseSummary, DownloadResult, ExportResult, RecordSummary } from "$lib/types";

  let {
    record,
    libraryPath = null,
    cases = [],
    selectedCaseId = null,
    onBack,
    onChanged
  } = $props<{
    record: RecordSummary;
    libraryPath?: string | null;
    cases: CaseSummary[];
    selectedCaseId: string | null;
    onBack: () => void;
    onChanged: () => void | Promise<void>;
  }>();

  function resolvePath(rel: string | null) {
    if (!rel || !libraryPath) return "";
    const cleanLib = libraryPath.endsWith("/") || libraryPath.endsWith("\\") ? libraryPath : libraryPath + "/";
    return convertFileSrc(cleanLib + rel);
  }

  let activeTab = $state<"overview" | "analysis" | "text" | "assets" | "case">("overview");
  let analysis = $state<AnalysisReport | null>(null);
  let busy = $state<string | null>(null);
  let error = $state<string | null>(null);
  let noteBody = $state("");
  let exportPath = $state<string | null>(null);

  const selectedCase = $derived(cases.find((item: CaseSummary) => item.id === selectedCaseId) ?? null);
  const imageAssets = $derived((analysis?.assets ?? []).filter((asset) => asset.asset_type === "image"));

  async function loadAnalysis() {
    error = null;
    try {
      analysis = await invoke<AnalysisReport | null>("get_analysis_result", { id: record.id });
    } catch (e) {
      error = String(e);
    }
  }

  async function download() {
    busy = "download";
    error = null;
    try {
      await invoke<DownloadResult>("download_record", { id: record.id });
      await onChanged();
      await loadAnalysis();
    } catch (e) {
      error = String(e);
    } finally {
      busy = null;
    }
  }

  async function analyze() {
    busy = "analysis";
    error = null;
    try {
      // Phase 1: Index (OCR, vectors, entities)
      await invoke<AnalysisReport>("index_record", { id: record.id });
      // Phase 2: Synthesize (Gemma neural intelligence)
      analysis = await invoke<AnalysisReport>("synthesize_intelligence", { id: record.id });
      await onChanged();
    } catch (e) {
      error = String(e);
    } finally {
      busy = null;
    }
  }

  async function openSource() {
    if (!record.document_url) return;
    await openUrl(record.document_url);
  }

  async function revealLocal() {
    if (!record.local_path) return;
    busy = "open-path";
    error = null;
    try {
      const path = await invoke<string>("get_record_artifact_path", { id: record.id });
      await openPath(path);
    } catch (e) {
      error = String(e);
    } finally {
      busy = null;
    }
  }

  async function addToCase() {
    if (!selectedCaseId) return;
    busy = "case-add";
    error = null;
    try {
      await invoke("add_record_to_case", {
        request: { case_id: selectedCaseId, record_id: record.id, notes: noteBody.trim() || null }
      });
      await onChanged();
    } catch (e) {
      error = String(e);
    } finally {
      busy = null;
    }
  }

  async function addNote() {
    if (!selectedCaseId || !noteBody.trim()) return;
    busy = "case-note";
    error = null;
    try {
      await invoke("update_case_notes", {
        request: { case_id: selectedCaseId, record_id: record.id, body: noteBody.trim() }
      });
      noteBody = "";
      await onChanged();
    } catch (e) {
      error = String(e);
    } finally {
      busy = null;
    }
  }

  async function exportCase(format: "markdown" | "html") {
    if (!selectedCaseId) return;
    busy = `export-${format}`;
    error = null;
    try {
      const result = await invoke<ExportResult>("export_case", {
        request: { case_id: selectedCaseId, format }
      });
      exportPath = result.absolute_path;
      await onChanged();
    } catch (e) {
      error = String(e);
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

  function short(value: string | null | undefined) {
    return value ? value.slice(0, 16) : "not recorded";
  }

  function formatJson(value: string | null | undefined) {
    if (!value) return "";
    try {
      return JSON.stringify(JSON.parse(value), null, 2);
    } catch {
      return value;
    }
  }

  $effect(() => {
    record.id;
    void loadAnalysis();
  });
</script>

<article class="archive-viewer">
  <header class="viewer-head">
    <div class="button-row">
      <button onclick={onBack}>Close</button>
      <button onclick={openSource} disabled={!record.document_url}>Source URL</button>
      <button onclick={revealLocal} disabled={!record.local_path || busy === "open-path"}>Open Local File</button>
    </div>

    <h2>{record.title}</h2>
    <p>{record.agency || "Unknown agency"} · {record.release_date || "undated"} · {record.incident_location || "no location"}</p>

    <div class="viewer-actions">
      <button class="primary" onclick={download} disabled={!!record.local_path || !record.document_url || busy === "download"}>
        {record.local_path ? "Downloaded" : busy === "download" ? "Downloading" : "Download"}
      </button>
      <button class="primary" onclick={analyze} disabled={!record.local_path || busy === "analysis"}>
        {busy === "analysis" ? "Analyzing..." : record.analysis_status === "completed" ? "Re-Audit" : record.analysis_status === "indexed" ? "Synthesize" : "Analyze"}
      </button>
    </div>
  </header>

  {#if error}
    <div class="notice error">
      <strong>Record action failed</strong>
      <span>{error}</span>
      <button onclick={() => (error = null)}>Dismiss</button>
    </div>
  {/if}

  <nav class="viewer-section tab-row" aria-label="Record detail tabs">
    <button class:active={activeTab === "overview"} onclick={() => (activeTab = "overview")}>Overview</button>
    <button class:active={activeTab === "analysis"} onclick={() => (activeTab = "analysis")}>Analysis</button>
    <button class:active={activeTab === "text"} onclick={() => (activeTab = "text")}>Text</button>
    <button class:active={activeTab === "assets"} onclick={() => (activeTab = "assets")}>Assets</button>
    <button class:active={activeTab === "case"} onclick={() => (activeTab = "case")}>Case</button>
  </nav>

  <div class="viewer-body">
    {#if activeTab === "overview"}
      <section class="viewer-section">
        <h3>Record Metadata</h3>
        <dl class="record-meta">
          <div><dt>Record ID</dt><dd>{record.id}</dd></div>
          <div><dt>Stable Key</dt><dd>{record.stable_key || "not recorded"}</dd></div>
          <div><dt>Source Type</dt><dd>{record.source_type}</dd></div>
          <div><dt>File Type</dt><dd>{record.file_type || "unknown"}</dd></div>
          <div><dt>Artifact Size</dt><dd>{formatBytes(record.artifact_size)}</dd></div>
          <div><dt>Artifact SHA-256</dt><dd>{short(record.artifact_sha256)}</dd></div>
          <div><dt>Content Hash</dt><dd>{short(record.content_hash)}</dd></div>
          <div><dt>Analysis Status</dt><dd class:status-completed={record.analysis_status === 'completed'} class:status-indexed={record.analysis_status === 'indexed'}>{record.analysis_status?.toUpperCase() || "PENDING"}</dd></div>
          <div><dt>Entities</dt><dd>{record.entity_count}</dd></div>
        </dl>
      </section>

      <section class="viewer-section">
        <h3>Summary</h3>
        <p>{record.summary || "No source summary supplied."}</p>
        {#if record.analysis_error}
          <p class="status failed">Last analysis error: {record.analysis_error}</p>
        {/if}
      </section>
    {:else if activeTab === "analysis"}
      <section class="viewer-section">
        <h3>Indexed Intelligence</h3>
        <dl class="record-meta">
          <div><dt>Status</dt><dd>{analysis?.status || record.analysis_status || "pending"}</dd></div>
          <div><dt>Engine</dt><dd>{analysis?.engine || "not run"}</dd></div>
          <div><dt>Chunks Indexed</dt><dd>{analysis?.chunks_indexed ?? 0}</dd></div>
          <div><dt>Redaction Score</dt><dd>{record.redaction_score == null ? "not analyzed" : `${(record.redaction_score * 100).toFixed(2)}%`}</dd></div>
        </dl>
      </section>

      <section class="viewer-section">
        <h3>Entities</h3>
        {#if analysis?.entities.length}
          <div class="entity-grid">
            {#each analysis.entities as entity}
              <span class="entity-pill">{entity.entity_type}: {entity.name} ({entity.confidence.toFixed(2)})</span>
            {/each}
          </div>
        {:else}
          <p>No entities indexed yet.</p>
        {/if}
      </section>

      <section class="viewer-section">
        <h3>Structured JSON</h3>
        {#if analysis?.intelligence_json || record.intelligence_json}
          <pre class="ocr-text">{formatJson(analysis?.intelligence_json || record.intelligence_json)}</pre>
        {:else}
          <p>No structured extraction stored.</p>
        {/if}
      </section>
    {:else if activeTab === "text"}
      <section class="viewer-section">
        <h3>Extracted Text</h3>
        {#if analysis?.ocr_text}
          <pre class="ocr-text">{analysis.ocr_text}</pre>
        {:else}
          <p>No extracted text stored. Download/import the artifact, then run analysis.</p>
        {/if}
      </section>
    {:else if activeTab === "assets"}
      <section class="viewer-section">
        <h3>Extracted Assets</h3>
        {#if imageAssets.length}
          <div class="asset-grid">
            {#each imageAssets as asset}
              <img src={resolvePath(asset.local_path)} alt="Extracted evidence asset" />
            {/each}
          </div>
        {:else}
          <p>No image assets extracted yet.</p>
        {/if}
      </section>
    {:else if activeTab === "case"}
      <section class="viewer-section">
        <h3>Case Work</h3>
        <p>{selectedCase ? `Selected case: ${selectedCase.title}` : "Create or select a case from the Cases view."}</p>
        <textarea bind:value={noteBody} rows="5" placeholder="Enter case analysis observations..."></textarea>
        <div class="button-row">
          <button onclick={addToCase} disabled={!selectedCaseId || busy === "case-add"}>Add Record</button>
          <button onclick={addNote} disabled={!selectedCaseId || !noteBody.trim() || busy === "case-note"}>Add Note</button>
          <button onclick={() => exportCase("markdown")} disabled={!selectedCaseId || busy === "export-markdown"}>Export MD</button>
          <button onclick={() => exportCase("html")} disabled={!selectedCaseId || busy === "export-html"}>Export HTML</button>
        </div>
        {#if exportPath}
          <p class="path-line">Export written: {exportPath}</p>
        {/if}
      </section>
    {/if}
  </div>
</article>
