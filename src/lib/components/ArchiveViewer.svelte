<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import type { AnalysisReport, CaseSummary, ExportResult, RecordSummary } from "$lib/types";

  let {
    record,
    cases = [],
    selectedCaseId = null,
    onBack,
    onChanged
  } = $props<{
    record: RecordSummary;
    cases: CaseSummary[];
    selectedCaseId: string | null;
    onBack: () => void;
    onChanged: (record: RecordSummary) => void | Promise<void>;
  }>();

  let analysis = $state<AnalysisReport | null>(null);
  let noteBody = $state("");
  let activeCaseId = $state<string>("");
  let busy = $state<string | null>(null);
  let message = $state<string | null>(null);
  let error = $state<string | null>(null);

  const selectedCase = $derived.by(() => cases.find((item: CaseSummary) => item.id === activeCaseId) ?? null);

  $effect(() => {
    if (selectedCaseId && !activeCaseId) activeCaseId = selectedCaseId;
  });

  async function loadAnalysis() {
    try {
      analysis = await invoke<AnalysisReport | null>("get_analysis_result", { id: record.id });
    } catch (caught) {
      error = String(caught);
    }
  }

  async function download() {
    busy = "download";
    error = null;
    try {
      await invoke("download_record", { id: record.id });
      const [updated] = await invoke<RecordSummary[]>("list_records", { filter: { query: record.title } });
      if (updated) await onChanged(updated);
      message = "Evidence stored in local library";
    } catch (caught) {
      error = String(caught);
    } finally {
      busy = null;
    }
  }

  async function analyze() {
    busy = "analysis";
    error = null;
    try {
      analysis = await invoke<AnalysisReport>("analyze_record", { id: record.id });
      const [updated] = await invoke<RecordSummary[]>("list_records", { filter: { query: record.title } });
      if (updated) await onChanged(updated);
      message = `Analysis complete with ${analysis.entities.length} entities`;
    } catch (caught) {
      error = String(caught);
    } finally {
      busy = null;
    }
  }

  async function addToCase() {
    if (!activeCaseId) return;
    busy = "case";
    error = null;
    try {
      await invoke("add_record_to_case", {
        request: { case_id: activeCaseId, record_id: record.id, notes: null }
      });
      await onChanged(record);
      message = "Record added to case";
    } catch (caught) {
      error = String(caught);
    } finally {
      busy = null;
    }
  }

  async function saveNote() {
    if (!activeCaseId || !noteBody.trim()) return;
    busy = "note";
    error = null;
    try {
      await invoke("update_case_notes", {
        request: { case_id: activeCaseId, record_id: record.id, body: noteBody.trim() }
      });
      noteBody = "";
      await onChanged(record);
      message = "Note saved";
    } catch (caught) {
      error = String(caught);
    } finally {
      busy = null;
    }
  }

  async function exportCase(format: "markdown" | "html") {
    if (!activeCaseId) return;
    busy = `export-${format}`;
    error = null;
    try {
      const result = await invoke<ExportResult>("export_case", {
        request: { case_id: activeCaseId, format }
      });
      message = `Export written: ${result.absolute_path}`;
    } catch (caught) {
      error = String(caught);
    } finally {
      busy = null;
    }
  }

  onMount(() => {
    void loadAnalysis();
  });
</script>

<div class="viewer">
  <header class="viewer-head">
    <button onclick={onBack}>← Index</button>
    <div>
      <h2>{record.title}</h2>
      <p>{record.source_type} · {record.agency || "UNKNOWN"} · {record.release_date || "N/A"}</p>
    </div>
    <div class="head-actions">
      {#if record.document_url}
        <a href={record.document_url} target="_blank" rel="noreferrer">Source</a>
      {/if}
      {#if !record.local_path && record.document_url}
        <button onclick={download} disabled={busy === "download"}>{busy === "download" ? "Getting" : "Download"}</button>
      {/if}
      <button class="primary" onclick={analyze} disabled={!record.local_path || busy === "analysis"}>
        {busy === "analysis" ? "Analyzing" : "Analyze"}
      </button>
    </div>
  </header>

  {#if error}<div class="banner error">{error}</div>{/if}
  {#if message}<div class="banner">{message}</div>{/if}

  <div class="viewer-grid">
    <section class="document-pane">
      <div class="document-sheet">
        {#if analysis?.ocr_text}
          <pre>{analysis.ocr_text}</pre>
        {:else}
          <div class="metadata">
            <h3>{record.file_type || "Evidence"}</h3>
            <dl>
              <dt>Record ID</dt><dd>{record.id}</dd>
              <dt>Incident</dt><dd>{record.incident_date || "N/A"}</dd>
              <dt>Location</dt><dd>{record.incident_location || "N/A"}</dd>
              <dt>Local Path</dt><dd>{record.local_path || "Not downloaded"}</dd>
              <dt>SHA-256</dt><dd>{record.artifact_sha256 || "Not stored"}</dd>
              <dt>Stable Key</dt><dd>{record.stable_key || "N/A"}</dd>
            </dl>
            <p>{record.summary || "No summary is attached to this record."}</p>
          </div>
        {/if}
      </div>
    </section>

    <aside class="inspector">
      <section>
        <h3>Entities</h3>
        <div class="entity-list">
          {#if analysis?.entities?.length}
            {#each analysis.entities as entity}
              <span title={entity.source}>{entity.entity_type}: {entity.name}</span>
            {/each}
          {:else}
            <small>No entity index yet</small>
          {/if}
        </div>
      </section>

      <section>
        <h3>Provenance</h3>
        <dl>
          <dt>Source</dt><dd>{record.source_type}</dd>
          <dt>Removed</dt><dd>{record.removed_from_source_at || "No"}</dd>
          <dt>Content Hash</dt><dd>{record.content_hash || "N/A"}</dd>
          <dt>Artifact Size</dt><dd>{record.artifact_size ? `${record.artifact_size} bytes` : "N/A"}</dd>
          <dt>Chunks</dt><dd>{analysis?.chunks_indexed ?? 0}</dd>
          <dt>Engine</dt><dd>{analysis?.engine || "N/A"}</dd>
        </dl>
      </section>

      <section>
        <h3>Case</h3>
        <select bind:value={activeCaseId}>
          <option value="">No case selected</option>
          {#each cases as item}
            <option value={item.id}>{item.title}</option>
          {/each}
        </select>
        <button onclick={addToCase} disabled={!activeCaseId || busy === "case"}>Add Record</button>
        {#if selectedCase}
          <small>{selectedCase.record_count} records · {selectedCase.note_count} notes</small>
        {/if}
      </section>

      <section>
        <h3>Notes</h3>
        <textarea bind:value={noteBody}></textarea>
        <button onclick={saveNote} disabled={!activeCaseId || !noteBody.trim() || busy === "note"}>Save Note</button>
      </section>

      <section>
        <h3>Dossier</h3>
        <div class="export-actions">
          <button onclick={() => exportCase("markdown")} disabled={!activeCaseId || busy === "export-markdown"}>Markdown</button>
          <button onclick={() => exportCase("html")} disabled={!activeCaseId || busy === "export-html"}>HTML</button>
        </div>
      </section>
    </aside>
  </div>
</div>

<style>
  .viewer {
    min-height: 0;
    height: 100%;
    display: flex;
    flex-direction: column;
    background: #101114;
  }

  .viewer-head {
    min-height: 72px;
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto;
    gap: 14px;
    align-items: center;
    padding: 12px 16px;
    border-bottom: 1px solid #303238;
    background: rgba(16, 17, 20, 0.94);
  }

  h2, h3, p {
    margin: 0;
  }

  h2 {
    font-size: 18px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .viewer-head p,
  small {
    color: #9da3ad;
    font-size: 12px;
  }

  button,
  a,
  select,
  textarea {
    font: inherit;
  }

  button,
  a {
    border: 1px solid #3a3d45;
    background: #1b1d22;
    color: #f4f1e8;
    border-radius: 6px;
    padding: 8px 11px;
    cursor: pointer;
    text-decoration: none;
  }

  button.primary {
    background: #e7c46b;
    color: #111;
    border-color: #e7c46b;
    font-weight: 800;
  }

  button:disabled {
    opacity: 0.55;
    cursor: not-allowed;
  }

  .head-actions,
  .export-actions {
    display: flex;
    gap: 8px;
    align-items: center;
  }

  .banner {
    margin: 10px 14px 0;
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

  .viewer-grid {
    min-height: 0;
    flex: 1;
    display: grid;
    grid-template-columns: minmax(0, 1fr) 360px;
  }

  .document-pane {
    min-width: 0;
    overflow: auto;
    padding: 28px;
    border-right: 1px solid #303238;
  }

  .document-sheet {
    min-height: 760px;
    max-width: 900px;
    margin: 0 auto;
    background: #f7f3e8;
    color: #171717;
    border-radius: 4px;
    box-shadow: 0 28px 70px rgba(0, 0, 0, 0.42);
    padding: 34px;
  }

  pre {
    margin: 0;
    white-space: pre-wrap;
    font: 13px/1.55 ui-monospace, SFMono-Regular, Menlo, monospace;
  }

  .metadata h3 {
    font-size: 28px;
    border-bottom: 3px solid #171717;
    padding-bottom: 14px;
    margin-bottom: 24px;
  }

  dl {
    display: grid;
    grid-template-columns: 110px minmax(0, 1fr);
    gap: 8px 14px;
    margin: 0;
  }

  dt {
    color: #727782;
    font-size: 11px;
    text-transform: uppercase;
  }

  dd {
    margin: 0;
    min-width: 0;
    overflow-wrap: anywhere;
  }

  .metadata p {
    margin-top: 26px;
    line-height: 1.6;
  }

  .inspector {
    min-height: 0;
    overflow-y: auto;
    background: rgba(20, 22, 27, 0.84);
    padding: 16px;
  }

  .inspector section {
    border: 1px solid #303238;
    background: rgba(9, 10, 12, 0.46);
    border-radius: 8px;
    padding: 13px;
    margin-bottom: 14px;
  }

  .inspector h3 {
    color: #e7c46b;
    font-size: 11px;
    text-transform: uppercase;
    margin-bottom: 10px;
  }

  .entity-list {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .entity-list span {
    border: 1px solid #3a3d45;
    border-radius: 999px;
    padding: 4px 8px;
    color: #dce4ee;
    font-size: 12px;
  }

  select,
  textarea {
    width: 100%;
    box-sizing: border-box;
    border: 1px solid #303238;
    background: #101114;
    color: #f4f1e8;
    border-radius: 6px;
    padding: 9px 10px;
    margin-bottom: 8px;
  }

  textarea {
    height: 118px;
    resize: vertical;
  }
</style>
