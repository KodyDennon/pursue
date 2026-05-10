<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { onMount, onDestroy } from "svelte";
  import { Brain, X, Loader2, CheckCircle2, AlertCircle, Terminal, Activity } from "lucide-svelte";
  import { addToast } from "$lib/toastStore";

  let { isOpen = $bindable(false), onComplete } = $props<{ 
    isOpen: boolean; 
    onComplete?: () => void;
  }>();

  let progress = $state(0);
  let status = $state("standby");
  let currentRecordId = $state<string | null>(null);
  let processedCount = $state(0);
  let totalCount = $state(0);
  let logs = $state<Array<{ time: string, msg: string, type: 'info' | 'error' | 'success' }>>([]);
  let busy = $state(false);

  function addLog(msg: string, type: 'info' | 'error' | 'success' = 'info') {
    const time = new Date().toLocaleTimeString([], { hour12: false, hour: '2-digit', minute: '2-digit', second: '2-digit' });
    logs = [{ time, msg, type }, ...logs].slice(0, 50);
  }

  async function startAnalysis() {
    if (busy) return;
    busy = true;
    status = "initializing";
    progress = 0;
    processedCount = 0;
    logs = [];
    addLog("Neural Extraction Engine starting...", "info");
    addLog("Mounting Gemma 4B Model (Int IT)...", "info");

    try {
      const count = await invoke<number>("analyze_all_records");
      totalCount = count;
      if (count === 0) {
        addLog("No pending records found. Archive is already up-to-date.", "success");
        status = "completed";
        busy = false;
        return;
      }
      addLog(`Task queued: ${count} records identified for deep extraction.`, "info");
      status = "processing";
    } catch (e) {
      addLog(`Initialization failed: ${e}`, "error");
      status = "failed";
      busy = false;
    }
  }

  onMount(() => {
    let unlisten: any;

    listen("analysis-progress", (event: any) => {
      const payload = event.payload;
      
      // Auto-activate and open modal if an event comes in from elsewhere
      if (payload.status === "starting" || payload.status === "processing" || payload.status === "analyzing" || payload.status === "thought") {
          if (!isOpen) isOpen = true;
          busy = true;
          status = payload.status === "analyzing" ? "processing" : payload.status === "thought" ? "reasoning" : payload.status;
      }
      
      processedCount = payload.current ?? processedCount;
      totalCount = payload.total ?? totalCount;
      currentRecordId = payload.record_id ?? currentRecordId;
      
      if (totalCount > 0) {
        progress = (processedCount / totalCount) * 100;
      }

      if (payload.status === "completed") {
        status = "completed";
        busy = false;
        addLog("Neural Extraction Task Complete.", "success");
        if (onComplete) onComplete();
      } else if (payload.status === "thought") {
        addLog(`Initiating step-by-step reasoning for ${payload.record_id.substring(0, 8)}...`, "info");
      } else if (payload.status === "failed") {
        status = "failed";
        busy = false;
        addLog(`System Error: ${payload.error}`, "error");
      } else if (payload.status === "record-failed") {
        addLog(`Record ${payload.record_id.substring(0, 8)} failed: ${payload.error}`, "error");
      } else if (currentRecordId) {
        addLog(`Processing record: ${currentRecordId.substring(0, 8)}...`, "info");
      }
    }).then(u => unlisten = u);

    return () => {
      if (unlisten) unlisten();
    };
  });

  function close() {
    if (busy) {
        if (!confirm("Analysis is running in the background. Close window?")) return;
    }
    isOpen = false;
  }
</script>

{#if isOpen}
  <div class="modal-overlay">
    <div class="analysis-panel glass-panel">
      <header class="panel-header">
        <div class="brand">
          <Brain size={24} class="accent-icon" />
          <div>
            <h2>Neural Extraction Engine</h2>
            <p>Gemma-powered automated intelligence synthesis.</p>
          </div>
        </div>
        <button class="close-btn" onclick={close}><X size={20} /></button>
      </header>

      <div class="panel-body">
        <section class="status-overview">
          <div class="progress-wrap">
            <div class="stats-row">
              <span class="status-label">{status.toUpperCase()}</span>
              <span class="count-label">{processedCount} / {totalCount} RECORDS</span>
            </div>
            <div class="progress-bar-bg">
              <div class="progress-bar-fill" style="width: {progress}%"></div>
              <div class="glow" style="left: {progress}%"></div>
            </div>
          </div>

          <div class="control-grid">
            <div class="info-card">
              <Activity size={18} />
              <div class="val">
                <span class="l">Current Unit</span>
                <span class="v">{currentRecordId ? currentRecordId.substring(0, 12) + '...' : 'None'}</span>
              </div>
            </div>
            <div class="info-card" class:thinking={status === 'reasoning'}>
              <Terminal size={18} />
              <div class="val">
                <span class="l">Status</span>
                <span class="v">{status === 'processing' ? 'EXTRACTING' : status === 'reasoning' ? 'THINKING' : status.toUpperCase()}</span>
              </div>
            </div>
            <button class="start-btn" onclick={startAnalysis} disabled={busy || status === 'completed'}>
               {#if busy}
                 <Loader2 size={18} class="spin" /> IN PROGRESS
               {:else if status === 'completed'}
                 <CheckCircle2 size={18} /> TASK COMPLETE
               {:else}
                 START BATCH PROCESS
               {/if}
            </button>
          </div>
        </section>

        <section class="log-section">
          <header>
            <Terminal size={14} /> <h3>Extraction Output Log</h3>
          </header>
          <div class="log-container custom-scrollbar">
            {#each logs as log}
              <div class="log-entry {log.type}">
                <span class="time">[{log.time}]</span>
                <span class="msg">{log.msg}</span>
              </div>
            {/each}
            {#if logs.length === 0}
               <div class="log-placeholder">Neural engine standby. Waiting for task initiation...</div>
            {/if}
          </div>
        </section>
      </div>

      <footer class="panel-footer">
        <div class="notice">
           <AlertCircle size={14} />
           <span>Intelligence extraction is hardware intensive. Do not close the application during active processing.</span>
        </div>
      </footer>
    </div>
  </div>
{/if}

<style>
  .modal-overlay {
    position: fixed;
    inset: 0;
    z-index: 2000;
    background: rgba(0,0,0,0.85);
    backdrop-filter: blur(10px);
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 40px;
  }

  .analysis-panel {
    width: 100%;
    max-width: 900px;
    height: 100%;
    max-height: 700px;
    background: #0a0b0d;
    border: 1px solid var(--border-subtle);
    display: flex;
    flex-direction: column;
    box-shadow: 0 30px 60px rgba(0,0,0,0.8);
    overflow: hidden;
  }

  .panel-header {
    padding: 24px 32px;
    display: flex;
    justify-content: space-between;
    align-items: center;
    border-bottom: 1px solid var(--border-subtle);
  }

  .brand {
    display: flex;
    gap: 20px;
    align-items: center;
  }

  .brand h2 { margin: 0; font-size: 20px; letter-spacing: 0.05em; }
  .brand p { margin: 4px 0 0 0; font-size: 13px; color: var(--text-secondary); }

  .close-btn {
    background: none;
    border: none;
    color: var(--text-tertiary);
    cursor: pointer;
    padding: 8px;
    border-radius: 50%;
    transition: all 0.2s;
  }

  .close-btn:hover { background: rgba(255,255,255,0.05); color: #fff; }

  .panel-body {
    flex: 1;
    padding: 32px;
    display: flex;
    flex-direction: column;
    gap: 32px;
    overflow: hidden;
  }

  .status-overview {
    display: flex;
    flex-direction: column;
    gap: 24px;
  }

  .progress-wrap {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .stats-row {
    display: flex;
    justify-content: space-between;
    font-size: 11px;
    font-weight: 800;
    letter-spacing: 0.1em;
  }

  .status-label { color: var(--accent-primary); }
  .count-label { color: var(--text-secondary); }

  .progress-bar-bg {
    height: 8px;
    background: rgba(255,255,255,0.05);
    border-radius: 4px;
    position: relative;
    overflow: hidden;
  }

  .progress-bar-fill {
    height: 100%;
    background: var(--accent-primary);
    box-shadow: 0 0 15px var(--accent-primary);
    transition: width 0.4s cubic-bezier(0.4, 0, 0.2, 1);
  }

  .progress-bar-bg .glow {
      position: absolute;
      top: 0;
      width: 100px;
      height: 100%;
      background: linear-gradient(90deg, transparent, rgba(231, 196, 107, 0.4), transparent);
      transform: translateX(-50%);
      transition: left 0.4s ease;
  }

  .control-grid {
    display: grid;
    grid-template-columns: 1fr 1fr 240px;
    gap: 16px;
  }

  .info-card {
    background: rgba(255,255,255,0.03);
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-md);
    padding: 16px;
    display: flex;
    align-items: center;
    gap: 16px;
    color: var(--text-secondary);
  }

  .info-card .val { display: flex; flex-direction: column; gap: 2px; }
  .info-card .l { font-size: 10px; text-transform: uppercase; font-weight: 700; opacity: 0.6; }
  .info-card .v { font-size: 14px; font-weight: 600; color: #fff; font-family: var(--font-mono); }

  .start-btn {
    background: var(--accent-primary);
    color: #000;
    border: none;
    border-radius: var(--radius-md);
    font-weight: 800;
    font-size: 13px;
    letter-spacing: 0.05em;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 10px;
    transition: all 0.2s;
  }

  .start-btn:hover:not(:disabled) { transform: scale(1.02); filter: brightness(1.1); }
  .start_btn:disabled { opacity: 0.5; cursor: not-allowed; }

  .info-card.thinking {
    border-color: #f3c46b;
    background: rgba(243, 196, 107, 0.05);
    animation: pulse-thought 2s infinite;
  }

  @keyframes pulse-thought {
    0% { box-shadow: 0 0 0 0 rgba(243, 196, 107, 0.2); }
    70% { box-shadow: 0 0 0 10px rgba(243, 196, 107, 0); }
    100% { box-shadow: 0 0 0 0 rgba(243, 196, 107, 0); }
  }

  .log-section {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 12px;
    overflow: hidden;
  }

  .log-section header {
    display: flex;
    align-items: center;
    gap: 10px;
    color: var(--text-secondary);
  }

  .log-section h3 { font-size: 12px; text-transform: uppercase; letter-spacing: 0.1em; margin: 0; }

  .log-container {
    flex: 1;
    background: rgba(0,0,0,0.4);
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-md);
    padding: 16px;
    font-family: var(--font-mono);
    font-size: 11px;
    display: flex;
    flex-direction: column;
    gap: 4px;
    overflow-y: auto;
  }

  .log-entry {
    display: flex;
    gap: 12px;
    line-height: 1.6;
  }

  .log-entry.info .time { color: var(--text-tertiary); }
  .log-entry.info .msg { color: var(--text-secondary); }
  .log-entry.success { background: rgba(77, 243, 169, 0.05); color: var(--accent-success); }
  .log-entry.error { background: rgba(243, 77, 77, 0.05); color: var(--accent-danger); }

  .log-placeholder {
    height: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-tertiary);
    font-style: italic;
    opacity: 0.5;
  }

  .panel-footer {
    padding: 20px 32px;
    background: rgba(0,0,0,0.3);
    border-top: 1px solid var(--border-subtle);
  }

  .notice {
    display: flex;
    align-items: center;
    gap: 12px;
    color: var(--text-tertiary);
    font-size: 11px;
  }

  .accent-icon { color: var(--accent-primary); }
  :global(.spin) { animation: spin 1s linear infinite; }
  @keyframes spin { from { transform: rotate(0deg); } to { transform: rotate(360deg); } }
</style>