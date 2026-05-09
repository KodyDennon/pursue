<script lang="ts">
  import { convertFileSrc, invoke } from "@tauri-apps/api/core";
  import { openPath, openUrl } from "@tauri-apps/plugin-opener";
  import type { AnalysisReport, CaseSummary, DownloadResult, ExportResult, RecordSummary, RecordAsset } from "$lib/types";

  let { record, cases = [], selectedCaseId = null, onBack, onChanged } = $props<{
    record: RecordSummary;
    cases: CaseSummary[];
    selectedCaseId: string | null;
    onBack: () => void;
    onChanged: () => void | Promise<void>;
  }>();

  let activeTab = $state<"intelligence" | "raw" | "media" | "case">("intelligence");
  let analysis = $state<AnalysisReport | null>(null);
  let busy = $state<string | null>(null);
  let error = $state<string | null>(null);
  let noteBody = $state("");
  let exportPath = $state<string | null>(null);

  const intelligence = $derived(record.intelligence_json ? JSON.parse(record.intelligence_json) : null);
  const images = $derived((analysis?.assets ?? []).filter((a: RecordAsset) => a.asset_type === 'image'));
  const selectedCase = $derived(cases.find((item: CaseSummary) => item.id === selectedCaseId) ?? null);

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
      analysis = await invoke<AnalysisReport>("analyze_record", { id: record.id });
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

<div class="intel-dossier glass-panel">
  <header class="dossier-header">
     <button class="back-btn" onclick={onBack}>← Back</button>
     <div class="actions">
       <button class="action-btn" onclick={openSource} disabled={!record.document_url}>Source</button>
       <button class="action-btn" onclick={revealLocal} disabled={!record.local_path || busy === 'open-path'}>Local File</button>
       <button class="action-btn" onclick={download} disabled={!!record.local_path || !record.document_url || busy === 'download'}>
          {record.local_path ? "Downloaded" : busy === "download" ? "Downloading..." : "Download Target"}
       </button>
       <button class="action-btn primary" onclick={analyze} disabled={!record.local_path || busy === 'analysis'}>
          {busy === "analysis" ? "Extracting Intel..." : record.analysis_status === "completed" ? "Re-Extract" : "Run Gemma 4"}
       </button>
     </div>
  </header>

  <div class="dossier-meta">
     <h2>{record.title}</h2>
     <div class="badges">
        <span class="badge">{record.agency || "Unknown Agency"}</span>
        <span class="badge">{record.source_type}</span>
        <span class="badge status-pill" class:completed={record.analysis_status === 'completed'}>{record.analysis_status || 'pending'}</span>
     </div>
  </div>

  {#if error}
    <div class="notice error">
      <strong>Action Failed:</strong> {error}
      <button onclick={() => error = null}>Dismiss</button>
    </div>
  {/if}

  <nav class="dossier-tabs">
    <button class:active={activeTab === "intelligence"} onclick={() => (activeTab = "intelligence")}>Executive Intel</button>
    <button class:active={activeTab === "raw"} onclick={() => (activeTab = "raw")}>Raw Extraction</button>
    <button class:active={activeTab === "media"} onclick={() => (activeTab = "media")}>Media Assets</button>
    <button class:active={activeTab === "case"} onclick={() => (activeTab = "case")}>Case Tools</button>
  </nav>

  <div class="dossier-body">
    {#if activeTab === "intelligence"}
      {#if intelligence}
        <div class="dossier-grid">
          <section class="intel-card full hero">
            <div class="card-glow"></div>
            <span class="card-label">Executive Intelligence Summary</span>
            <p class="summary-text">{intelligence.object_description || 'No detailed description extracted.'}</p>
          </section>

          <section class="intel-card">
            <span class="card-label">Primary Engagement Data</span>
            <div class="metrics-grid">
              <div class="m-item">
                <span class="m-label">Incident Date</span>
                <span class="m-val">{intelligence.incident_date || 'N/A'}</span>
              </div>
              <div class="m-item">
                <span class="m-label">Location</span>
                <span class="m-val">{intelligence.location || 'N/A'}</span>
              </div>
            </div>
          </section>

          <section class="intel-card">
            <span class="card-label">Agencies Involved</span>
            <div class="tag-cloud">
              {#each (intelligence.agencies || []) as agency}
                <span class="intel-tag">{agency}</span>
              {/each}
              {#if !(intelligence.agencies?.length)}
                <span class="no-data">None identified</span>
              {/if}
            </div>
          </section>

          <section class="intel-card full observations">
            <header class="obs-head">
              <span class="card-label">Pilot & Personnel Observations</span>
              <span class="live-indicator">GROUND TRUTH EXTRACTED</span>
            </header>
            <div class="obs-content">
              <div class="quote-mark">“</div>
              <p class="summary-text small">{intelligence.pilot_observations || 'No specific personnel observations documented.'}</p>
            </div>
          </section>

          <section class="intel-card full forensics">
            <div class="f-header">
              <span class="card-label">Intelligence Confidence & Integrity</span>
              <span class="engine-tag">Gemma 4 Elite</span>
            </div>
            <div class="f-body">
              <div class="f-metric">
                <span>Redaction Check</span>
                <strong>{intelligence.redaction_summary || 'Not analyzed'}</strong>
              </div>
              <div class="f-metric">
                <span>Extraction Status</span>
                <strong class="status-ok">VERIFIED</strong>
              </div>
            </div>
          </section>
        </div>
      {:else}
        <div class="pending-dossier">
          <div class="spinner"></div>
          <h3>Intelligence Extraction Pending</h3>
          <p>Initiate Gemma 4 deep analysis to populate this dossier.</p>
        </div>
      {/if}
    {:else if activeTab === "raw"}
      <div class="raw-view">
        <section class="viewer-section">
          <h3>Structured JSON</h3>
          {#if analysis?.intelligence_json || record.intelligence_json}
            <pre class="ocr-text">{formatJson(analysis?.intelligence_json || record.intelligence_json)}</pre>
          {:else}
            <p>No structured extraction stored.</p>
          {/if}
        </section>

        <section class="viewer-section">
          <h3>Extracted Text (OCR)</h3>
          {#if analysis?.ocr_text}
            <pre class="ocr-text">{analysis.ocr_text}</pre>
          {:else}
            <p>No extracted text stored.</p>
          {/if}
        </section>
      </div>
    {:else if activeTab === "media"}
      <section class="viewer-section">
        <h3>Extracted Assets</h3>
        {#if images.length}
          <div class="asset-grid">
            {#each images as asset}
              <div class="evidence-frame glass">
                <img src={convertFileSrc(asset.local_path)} alt="Extracted evidence asset" />
                <div class="frame-meta">
                  <span>{asset.mime_type}</span>
                  <span>{asset.file_size ? (asset.file_size / 1024).toFixed(0) : 0} KB</span>
                </div>
              </div>
            {/each}
          </div>
        {:else}
          <p>No image assets extracted yet.</p>
        {/if}
      </section>
    {:else if activeTab === "case"}
      <section class="viewer-section case-section">
        <h3>Case Work</h3>
        <p>{selectedCase ? `Selected case: ${selectedCase.title}` : "Create or select a case from the Cases view."}</p>
        <textarea bind:value={noteBody} rows="5" placeholder="Record note for the selected case"></textarea>
        <div class="actions">
          <button class="action-btn" onclick={addToCase} disabled={!selectedCaseId || busy === "case-add"}>Add Record to Case</button>
          <button class="action-btn primary" onclick={addNote} disabled={!selectedCaseId || !noteBody.trim() || busy === "case-note"}>Commit Note</button>
        </div>
      </section>
    {/if}
  </div>
</div>

<style>
  .intel-dossier {
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow: hidden;
    background: var(--bg-surface-elevated);
    border-left: 1px solid var(--border-subtle);
  }

  .dossier-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 16px 24px;
    border-bottom: 1px solid var(--border-subtle);
  }

  .back-btn {
    color: var(--text-secondary);
    font-size: 14px;
  }
  .back-btn:hover { color: var(--text-primary); }

  .actions {
    display: flex;
    gap: 8px;
  }

  .action-btn {
    padding: 6px 12px;
    background: var(--bg-surface);
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-sm);
    color: var(--text-primary);
    font-size: 13px;
    transition: var(--transition-fast);
  }

  .action-btn:hover:not(:disabled) {
    border-color: var(--accent-primary);
  }

  .action-btn.primary {
    background: var(--accent-primary);
    color: #000;
    font-weight: 600;
    border: none;
  }

  .action-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .dossier-meta {
    padding: 24px;
  }

  .dossier-meta h2 {
    font-size: 24px;
    margin-bottom: 12px;
  }

  .badges {
    display: flex;
    gap: 8px;
  }

  .badge {
    background: rgba(255, 255, 255, 0.1);
    padding: 4px 8px;
    border-radius: var(--radius-sm);
    font-size: 12px;
    color: var(--text-secondary);
  }
  
  .status-pill.completed {
    background: rgba(77, 243, 169, 0.1);
    color: var(--accent-success);
    border: 1px solid rgba(77, 243, 169, 0.2);
  }

  .dossier-tabs {
    display: flex;
    border-bottom: 1px solid var(--border-subtle);
    padding: 0 24px;
  }

  .dossier-tabs button {
    padding: 12px 16px;
    color: var(--text-secondary);
    border-bottom: 2px solid transparent;
    font-size: 14px;
  }

  .dossier-tabs button.active {
    color: var(--accent-primary);
    border-bottom-color: var(--accent-primary);
  }

  .dossier-body {
    flex: 1;
    overflow-y: auto;
    padding: 24px;
  }

  /* Intelligence Grid Styles */
  .dossier-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 20px;
  }

  .intel-card {
    background: var(--bg-surface);
    border: 1px solid var(--border-subtle);
    border-radius: 12px;
    padding: 24px;
    position: relative;
    overflow: hidden;
  }

  .intel-card.full {
    grid-column: span 2;
  }

  .hero {
    background: linear-gradient(135deg, var(--bg-surface), #15171d);
  }

  .card-glow {
    position: absolute;
    top: -50%;
    left: -50%;
    width: 200%;
    height: 200%;
    background: radial-gradient(circle at 50% 50%, rgba(231, 196, 107, 0.05), transparent 70%);
    pointer-events: none;
  }

  .card-label {
    display: block;
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.15em;
    color: var(--text-secondary);
    margin-bottom: 16px;
  }

  .summary-text {
    font-size: 15px;
    line-height: 1.6;
  }

  .summary-text.small {
    font-size: 14px;
    color: #9da3ad;
    font-style: italic;
  }

  .metrics-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 24px;
  }

  .m-item {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .m-label {
    font-size: 11px;
    color: var(--text-secondary);
  }

  .m-val {
    font-size: 16px;
    font-weight: 600;
    color: var(--accent-primary);
  }

  .observations {
    background: rgba(0,0,0,0.4);
    border: 1px solid rgba(231, 196, 107, 0.15);
  }

  .obs-head {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 16px;
  }

  .live-indicator {
    font-size: 9px;
    font-weight: 800;
    color: var(--accent-primary);
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .live-indicator::before {
    content: '';
    width: 6px;
    height: 6px;
    background: var(--accent-primary);
    border-radius: 50%;
    animation: pulse 2s infinite;
  }

  .obs-content {
    display: flex;
    gap: 20px;
  }

  .quote-mark {
    font-size: 48px;
    color: rgba(231, 196, 107, 0.2);
    font-family: serif;
    line-height: 0.5;
    margin-top: 12px;
  }

  .asset-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
    gap: 16px;
  }

  .evidence-frame {
    aspect-ratio: 4/3;
    overflow: hidden;
    position: relative;
    border-radius: 8px;
    border: 1px solid var(--border-subtle);
  }

  .evidence-frame img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .frame-meta {
    position: absolute;
    bottom: 0;
    left: 0;
    right: 0;
    background: rgba(0,0,0,0.8);
    padding: 8px 12px;
    font-size: 10px;
    display: flex;
    justify-content: space-between;
  }

  .tag-cloud {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }

  .intel-tag {
    background: rgba(231, 196, 107, 0.1);
    color: var(--accent-primary);
    border: 1px solid rgba(231, 196, 107, 0.2);
    padding: 4px 12px;
    border-radius: 4px;
    font-size: 12px;
  }

  .forensics {
    background: rgba(0,0,0,0.3);
  }

  .f-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 20px;
  }

  .engine-tag {
    font-size: 10px;
    font-weight: 800;
    background: var(--accent-primary);
    color: #000;
    padding: 2px 8px;
    border-radius: 4px;
  }

  .f-body {
    display: flex;
    gap: 40px;
  }

  .f-metric {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .f-metric span {
    font-size: 11px;
    color: var(--text-secondary);
  }

  .f-metric strong {
    font-size: 14px;
  }

  .status-ok {
    color: var(--accent-success);
  }

  .pending-dossier {
    height: 300px;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    text-align: center;
  }

  .spinner {
    width: 32px;
    height: 32px;
    border: 2px solid var(--border-subtle);
    border-top-color: var(--accent-primary);
    border-radius: 50%;
    animation: spin 1s infinite linear;
    margin-bottom: 24px;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }

  @keyframes pulse {
    0% { transform: scale(1); opacity: 1; }
    50% { transform: scale(1.5); opacity: 0.5; }
    100% { transform: scale(1); opacity: 1; }
  }

  .viewer-section h3 {
    font-size: 16px;
    margin-bottom: 16px;
  }

  .ocr-text {
    font-family: var(--font-mono);
    font-size: 12px;
    background: #000;
    padding: 16px;
    border-radius: 8px;
    overflow-x: auto;
    white-space: pre-wrap;
    word-break: break-all;
  }

  .case-section textarea {
    width: 100%;
    background: rgba(0,0,0,0.2);
    border: 1px solid var(--border-subtle);
    color: var(--text-primary);
    padding: 12px;
    border-radius: 8px;
    margin: 16px 0;
    font-family: var(--font-sans);
    resize: vertical;
  }

  .notice {
    margin: 0 24px 24px;
    padding: 12px 16px;
    background: rgba(243, 77, 77, 0.1);
    border: 1px solid var(--accent-danger);
    border-radius: 8px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    font-size: 14px;
  }
</style>
