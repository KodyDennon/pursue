<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import type { AnalysisReport, CaseSummary, ExportResult, RecordSummary } from "$lib/types";
  import IntelligenceDossier from "./IntelligenceDossier.svelte";

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

  let activeTab = $state<"intelligence" | "document" | "metadata" | "forensics">("intelligence");
  let analysis = $state<AnalysisReport | null>(null);
  let busy = $state<string | null>(null);
  let selectedTier = $state<"Draft" | "Deep" | "Elite">("Deep");

  async function loadAnalysis() {
    try {
      analysis = await invoke<AnalysisReport | null>("get_analysis_result", { id: record.id });
    } catch (e) {
      console.error(e);
    }
  }

  async function analyze() {
    busy = "analysis";
    try {
      analysis = await invoke<AnalysisReport>("analyze_record", { id: record.id });
      onChanged(record);
    } catch (e) {
      alert(e);
    } finally {
      busy = null;
    }
  }

  onMount(() => {
    void loadAnalysis();
  });
</script>

<div class="dossier-viewer glass">
  <header class="dossier-head">
    <button class="back-btn" onclick={onBack}>← Back to Terminal</button>
    <div class="title-block">
      <h1>{record.title}</h1>
      <p>{record.agency} · Incident ID: {record.stable_key || 'UNKNOWN'}</p>
    </div>
    <div class="dossier-actions">
      <select bind:value={selectedTier} class="tier-select">
        <option value="Draft">Draft</option>
        <option value="Deep">Deep (E2B)</option>
        <option value="Elite">Elite (26B)</option>
      </select>
      <button class="primary" onclick={analyze} disabled={busy === 'analysis'}>
        {busy === 'analysis' ? 'Running Gemma 4...' : 'Analyze Intelligence'}
      </button>
    </div>
  </header>

  <nav class="dossier-tabs">
    <button class:active={activeTab === 'intelligence'} onclick={() => activeTab = 'intelligence'}>Intelligence</button>
    <button class:active={activeTab === 'document'} onclick={() => activeTab = 'document'}>Evidence Sheet</button>
    <button class:active={activeTab === 'metadata'} onclick={() => activeTab = 'metadata'}>Registry</button>
    <button class:active={activeTab === 'forensics'} onclick={() => activeTab = 'forensics'}>Forensics</button>
  </nav>

  <div class="dossier-content">
    {#if activeTab === 'intelligence'}
      <IntelligenceDossier {record} assets={analysis?.assets || []} />
    {:else if activeTab === 'document'}
      <div class="evidence-container">
        <div class="document-sheet glass">
          <header class="sheet-head">
            <div class="h-left">
              <span class="classification">UNCLASSIFIED // FOUO</span>
              <span class="artifact-id">REF: {record.stable_key || 'N/A'}</span>
            </div>
            <div class="h-right">
              <span class="stamp">OFFICIAL PURSUE DIGITIZATION</span>
            </div>
          </header>
          {#if analysis?.ocr_text}
            <div class="digitized-text">
              {@html analysis.ocr_text.split('\n').map(line => `<p>${line}</p>`).join('')}
            </div>
          {:else}
            <div class="empty-doc">
              <div class="scan-line"></div>
              <p>Digitization required. Run Gemma 4 Analysis to populate evidence sheet.</p>
            </div>
          {/if}
          <footer class="sheet-foot">
            <div class="foot-meta">
              <span>SCAN_VERIFIED: {analysis?.engine || 'PENDING'}</span>
              <span>SHA256: {record.artifact_sha256?.slice(0, 16)}...</span>
            </div>
            <p>PURSUE Intelligence Platform · Ground Truth Extraction · Automated Forensic Digitization</p>
          </footer>
        </div>
      </div>
    {:else if activeTab === 'metadata'}
      <div class="metadata-grid">
        <div class="meta-item">
          <label>Source Agency</label>
          <span>{record.agency}</span>
        </div>
        <div class="meta-item">
          <label>Release Date</label>
          <span>{record.release_date || 'Classified'}</span>
        </div>
        <div class="meta-item">
          <label>SHA-256 Hash</label>
          <code class="hash">{record.artifact_sha256 || 'Unverified'}</code>
        </div>
        <div class="meta-item">
          <label>Stable Key</label>
          <span>{record.stable_key}</span>
        </div>
        <div class="meta-item">
          <label>Provenance</label>
          <span>Official Department of War Source (WAR.GOV)</span>
        </div>
      </div>
    {:else if activeTab === 'forensics'}
      <div class="forensics-panel glass">
        <div class="forensics-header">
          <h3>Forensic Density Analysis</h3>
          <p>Analyzing document for withheld or redacted information.</p>
        </div>
        <div class="metric-group">
          <div class="metric">
            <div class="m-head">
              <label>Redaction Density</label>
              <span>{(record.redaction_score || 0).toFixed(2)}%</span>
            </div>
            <div class="progress-bar">
              <div class="fill" style="width: {(record.redaction_score || 0) * 100}%"></div>
            </div>
          </div>
          <div class="metric">
            <label>Analysis Status</label>
            <span class="value status-{record.analysis_status}">{record.analysis_status || 'Pending'}</span>
          </div>
          <div class="metric">
            <label>Intelligence Engine</label>
            <span class="value">Gemma 4 (Local Metal Inference)</span>
          </div>
        </div>
      </div>
    {/if}
  </div>
</div>

<style>
  .dossier-viewer {
    height: 100%;
    display: flex;
    flex-direction: column;
    border-radius: 24px;
    border: 1px solid var(--border-dim);
    overflow: hidden;
  }

  .dossier-head {
    padding: 32px;
    display: grid;
    grid-template-columns: auto 1fr auto;
    gap: 32px;
    align-items: center;
    border-bottom: 1px solid var(--border-dim);
  }

  .back-btn {
    background: transparent;
    border-color: var(--border-dim);
    color: var(--text-secondary);
  }

  h1 {
    margin: 0;
    font-size: 28px;
    font-weight: 800;
  }

  .title-block p {
    margin: 4px 0 0;
    color: var(--text-secondary);
    font-size: 14px;
  }

  .dossier-actions {
    display: flex;
    gap: 12px;
  }

  .tier-select {
    background: var(--bg-secondary);
    color: white;
    border: 1px solid var(--border-dim);
    border-radius: 8px;
    padding: 0 12px;
    outline: none;
  }

  .dossier-tabs {
    display: flex;
    padding: 0 32px;
    background: rgba(0,0,0,0.2);
    border-bottom: 1px solid var(--border-dim);
  }

  .dossier-tabs button {
    background: transparent;
    border: none;
    border-bottom: 2px solid transparent;
    border-radius: 0;
    padding: 16px 24px;
    color: var(--text-secondary);
  }

  .dossier-tabs button.active {
    color: var(--accent-gold);
    border-bottom-color: var(--accent-gold);
  }

  .dossier-content {
    flex: 1;
    overflow-y: auto;
    padding: 40px;
    background: radial-gradient(circle at top right, rgba(231, 196, 107, 0.05), transparent);
  }

  .evidence-container {
    padding: 20px;
    background: #000;
    border-radius: 12px;
    box-shadow: inset 0 0 40px rgba(0,0,0,1);
  }

  .document-sheet {
    background: #fdfaf3;
    color: #1a1a1a;
    padding: 60px 80px;
    border-radius: 2px;
    box-shadow: 0 40px 100px rgba(0,0,0,0.8);
    max-width: 850px;
    margin: 0 auto;
    min-height: 1100px;
    position: relative;
    border: 1px solid #dcd3b6;
  }

  .sheet-head {
    display: flex;
    justify-content: space-between;
    font-family: 'JetBrains Mono', monospace;
    font-size: 10px;
    border-bottom: 2px solid #1a1a1a;
    padding-bottom: 12px;
    margin-bottom: 40px;
    color: #555;
  }

  .classification {
    display: block;
    font-weight: 800;
    color: #000;
    font-size: 11px;
    margin-bottom: 4px;
  }

  .stamp {
    border: 2px solid #dcd3b6;
    padding: 4px 8px;
    color: #dcd3b6;
    font-size: 9px;
    font-weight: 800;
    text-transform: uppercase;
    transform: rotate(-5deg);
    display: inline-block;
  }

  .sheet-foot {
    position: absolute;
    bottom: 40px;
    left: 80px;
    right: 80px;
    border-top: 1px solid #ccc;
    padding-top: 12px;
    font-size: 9px;
    color: #888;
    text-align: center;
  }

  .foot-meta {
    display: flex;
    justify-content: space-between;
    margin-bottom: 8px;
    font-family: 'JetBrains Mono', monospace;
  }

  .digitized-text {
    font-family: 'JetBrains Mono', monospace;
    font-size: 13px;
    line-height: 1.6;
    color: #1a1a1a;
  }

  .digitized-text p {
    margin-bottom: 1em;
  }

  .scan-line {
    width: 100%;
    height: 2px;
    background: var(--accent-gold);
    box-shadow: 0 0 15px var(--accent-gold);
    animation: scan 3s infinite linear;
    position: absolute;
    top: 0;
  }

  @keyframes scan {
    from { top: 10%; opacity: 0; }
    50% { opacity: 1; }
    to { top: 90%; opacity: 0; }
  }

  .metadata-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
    gap: 24px;
  }

  .meta-item {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 20px;
    background: var(--bg-secondary);
    border-radius: 12px;
    border: 1px solid var(--border-dim);
  }

  .meta-item label {
    font-size: 11px;
    text-transform: uppercase;
    color: var(--text-secondary);
    letter-spacing: 0.1em;
  }

  .hash {
    font-size: 12px;
    color: var(--accent-gold);
    word-break: break-all;
  }

  .forensics-panel {
    padding: 32px;
    display: flex;
    flex-direction: column;
    gap: 32px;
  }

  .forensics-header h3 {
    margin: 0 0 8px;
    font-size: 20px;
  }

  .forensics-header p {
    margin: 0;
    color: var(--text-secondary);
    font-size: 14px;
  }

  .metric-group {
    display: flex;
    flex-direction: column;
    gap: 24px;
  }

  .metric {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .m-head {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .progress-bar {
    height: 8px;
    background: var(--bg-primary);
    border-radius: 4px;
    overflow: hidden;
  }

  .fill {
    height: 100%;
    background: var(--accent-gold);
    box-shadow: 0 0 10px var(--accent-gold);
  }

  .status-completed { color: #4df3a9; }
  .status-pending { color: var(--text-secondary); }

  .empty-doc {
    height: 400px;
    display: grid;
    place-items: center;
    color: #9da3ad;
  }
</style>
