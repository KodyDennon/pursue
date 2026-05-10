<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onMount, onDestroy } from "svelte";
  import { Brain, Cpu, Database, HardDrive, Download, CheckCircle2, AlertCircle, Loader2 } from "lucide-svelte";
  import type { DatabaseStatus } from "$lib/types";
  import { addToast } from "$lib/toastStore";

  import { listen } from "@tauri-apps/api/event";

  let { onAnalyze } = $props<{ onAnalyze?: () => void }>();

  let status = $state<DatabaseStatus | null>(null);
  let diagnostics = $state<any>(null);
  let models = $state([
    { id: 'bge-small', name: 'BGE Small v1.5', filename: 'bge-small-en-v1.5.onnx', type: 'Embedding', size: '134 MB', status: 'pending', progress: 0, url: 'https://huggingface.co/BAAI/bge-small-en-v1.5/resolve/main/onnx/model.onnx' },
    { id: 'tokenizer', name: 'BGE Tokenizer', filename: 'tokenizer.json', type: 'System', size: '1 MB', status: 'pending', progress: 0, url: 'https://huggingface.co/BAAI/bge-small-en-v1.5/resolve/main/tokenizer.json' },
    { id: 'gemma-4-e2b', name: 'Gemma 4 E2B IT', filename: 'google/gemma-4-E2B-it', type: 'Intelligence', size: '10.2 GB', status: 'pending', progress: 0, url: 'google/gemma-4-E2B-it' },
    { id: 'gemma-4-e4b', name: 'Gemma 4 E4B IT', filename: 'google/gemma-4-E4B-it', type: 'Intelligence (Elite)', size: '16.0 GB', status: 'pending', progress: 0, url: 'google/gemma-4-E4B-it' }
  ]);

  let busyModelId = $state<string | null>(null);

  let analysisProgress = $state(0);
  let analysisActive = $state(false);
  let analysisStatus = $state("");

  async function loadStatus() {
    try {
      status = await invoke<DatabaseStatus>("get_database_status");
      diagnostics = await invoke<any>("get_hardware_diagnostics");
      const modelStatus = await invoke<Record<string, boolean>>("check_model_status");
      
      models = models.map(m => ({
        ...m,
        status: modelStatus[m.id] ? "ready" : (busyModelId === m.id ? "downloading" : "missing")
      }));
    } catch (e) {
      console.error(e);
    }
  }

  async function downloadModel(modelId: string) {
    busyModelId = modelId;
    const model = models.find(m => m.id === modelId);
    if (!model) return;

    try {
      addToast({ type: "info", message: `Provisioning ${model.name}...`, duration: 3000 });
      await invoke("provision_model", { id: model.id, url: model.url, name: model.filename });
      await loadStatus();
      addToast({ type: "success", message: `${model.name} is ready.`, duration: 3000 });
    } catch (e) {
      addToast({ type: "error", message: `Provisioning failed: ${e}` });
    } finally {
      busyModelId = null;
    }
  }

  async function provisionAll() {
    const missing = models.filter(m => m.status === 'missing');
    for (const model of missing) {
      await downloadModel(model.id);
    }
  }

  async function reindexAll() {
    if (analysisActive) return;
    try {
      if (onAnalyze) onAnalyze();
      analysisActive = true;
      analysisProgress = 0;
      analysisStatus = "Initializing...";
      const count = await invoke<number>("analyze_all_records");
      addToast({ type: "info", message: `Neural Indexing initiated for ${count} records.`, duration: 5000 });
    } catch (e) {
      addToast({ type: "error", message: `Indexing failed: ${e}`, duration: 5000 });
      analysisActive = false;
    }
  }

  onMount(() => {
    loadStatus();
    const interval = setInterval(loadStatus, 5000);
    
    let unlistenProgress: Promise<() => void>;
    let unlistenAnalysis: Promise<() => void>;

    unlistenProgress = listen("model-progress", (event: any) => {
      const payload = event.payload;
      const model = models.find(m => m.id === payload.model_id);
      if (model) {
        model.status = payload.status;
        if (payload.total_bytes) {
          model.progress = (payload.bytes_downloaded / payload.total_bytes) * 100;
        }
      }
    });

    unlistenAnalysis = listen("analysis-progress", (event: any) => {
      const { current, total, status } = event.payload;
      if (total > 0) {
        analysisProgress = (current / total) * 100;
      }
      if (status === "completed") {
        analysisActive = false;
        analysisStatus = "Complete";
        loadStatus();
      } else {
        analysisActive = true;
        analysisStatus = `Processing ${current} of ${total}`;
      }
    });

    return () => {
      clearInterval(interval);
      if (unlistenProgress) unlistenProgress.then(u => u());
      if (unlistenAnalysis) unlistenAnalysis.then(u => u());
    };
  });
</script>

<div class="intelligence-center custom-scrollbar">
  <header class="page-header">
    <div class="title-wrap">
      <Brain class="accent-icon" size={32} />
      <div>
        <h1>Neural Engine</h1>
        <p>Coordinate neural models, vector indices, and hardware acceleration.</p>
      </div>
    </div>
  </header>

  <div class="center-grid">
    <!-- Hardware Diagnostics -->
    <section class="center-card diagnostics">
      <header>
        <Cpu size={18} />
        <h3>Hardware Diagnostics</h3>
      </header>
      {#if diagnostics}
        <div class="diag-metrics">
          <div class="metric">
            <span>Processor</span>
            <strong>{diagnostics.cpu_brand || 'Generic CPU'}</strong>
          </div>
          <div class="metric">
            <span>Memory Pool</span>
            <strong>{diagnostics.total_memory_gb} GB Total</strong>
          </div>
          <div class="metric">
            <span>Acceleration</span>
            <strong class={diagnostics.gpu_acceleration_available ? 'text-success' : 'text-warning'}>
              {diagnostics.gpu_acceleration_available ? 'GPU Active (Metal/CUDA)' : 'CPU Only (Fallback)'}
            </strong>
          </div>
          <div class="metric">
            <span>Intelligence Tier</span>
            <strong class="tier-badge {diagnostics.recommended_tier}">
              {diagnostics.recommended_tier}
            </strong>
          </div>
        </div>
      {:else}
        <div class="loading-state">Probing hardware...</div>
      {/if}
    </section>

    <!-- Model Management -->
    <section class="center-card models">
      <header>
        <Database size={18} />
        <div class="header-content">
          <h3>Cognitive Models</h3>
          {#if models.some(m => m.status === 'missing')}
            <button class="text-btn" onclick={provisionAll} disabled={!!busyModelId}>
              <Download size={14} /> Provision All Missing
            </button>
          {/if}
        </div>
      </header>
      <div class="model-list">
        {#each models as model}
          <div class="model-item" class:busy={busyModelId === model.id}>
            <div class="model-info">
              <span class="m-type">{model.type}</span>
              <span class="m-name">{model.name}</span>
              {#if model.status === 'downloading'}
                <div class="progress-container">
                  <div class="progress-bar" style="width: {model.progress}%"></div>
                  <span class="m-size">{model.progress.toFixed(1)}% of {model.size}</span>
                </div>
              {:else}
                <span class="m-size">{model.size} • {model.status}</span>
              {/if}
            </div>
            <div class="model-actions">
              {#if busyModelId === model.id}
                <Loader2 class="spin" size={18} />
              {:else if model.status === 'ready'}
                <CheckCircle2 class="text-success" size={18} />
              {:else}
                <button class="icon-btn" onclick={() => downloadModel(model.id)}>
                  <Download size={18} />
                </button>
              {/if}
            </div>
          </div>
        {/each}
      </div>
    </section>

    <!-- Vector Database Status -->
    <section class="center-card vector">
      <header>
        <HardDrive size={18} />
        <div class="header-content">
          <h3>Vector Index Analytics</h3>
          {#if analysisActive}
            <div class="analysis-progress">
              <span class="status-text">{analysisStatus}</span>
              <div class="progress-bar-bg">
                <div class="progress-bar-fill" style="width: {analysisProgress}%"></div>
              </div>
            </div>
          {:else}
            <button class="text-btn" onclick={reindexAll}>
              <Brain size={14} /> Batch Neural Re-indexing
            </button>
          {/if}
        </div>
      </header>
      {#if status}
        <div class="diag-metrics">
          <div class="metric">
            <span>Indexed Chunks</span>
            <strong>{status.vector_chunks}</strong>
          </div>
          <div class="metric">
            <span>Entity Associations</span>
            <strong>{status.entity_count}</strong>
          </div>
          <div class="metric">
            <span>Storage Overhead</span>
            <strong>{(status.artifact_bytes / 1024 / 1024).toFixed(1)} MB</strong>
          </div>
          <div class="metric">
            <span>Search Engine</span>
            <strong>ONNX Runtime v1.17</strong>
          </div>
        </div>
      {:else}
        <div class="loading-state">Syncing index status...</div>
      {/if}
    </section>
  </div>
</div>

<style>
  .intelligence-center {
    height: 100%;
    overflow-y: auto;
    padding: 40px;
    display: flex;
    flex-direction: column;
    gap: 40px;
  }

  .page-header {
    display: flex;
    justify-content: space-between;
    align-items: flex-end;
  }

  .title-wrap {
    display: flex;
    gap: 20px;
    align-items: center;
  }

  .title-wrap h1 {
    font-size: 32px;
    margin: 0;
    font-weight: 700;
  }

  .title-wrap p {
    color: var(--text-secondary);
    margin: 4px 0 0 0;
  }

  .accent-icon { color: var(--accent-primary); }

  .center-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(400px, 1fr));
    gap: 24px;
  }

  .center-card {
    background: var(--bg-surface);
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-lg);
    padding: 24px;
    display: flex;
    flex-direction: column;
    gap: 20px;
  }

  .center-card header {
    display: flex;
    align-items: center;
    gap: 12px;
    color: var(--text-secondary);
  }

  .center-card h3 {
    margin: 0;
    font-size: 14px;
    text-transform: uppercase;
    letter-spacing: 0.1em;
    font-weight: 700;
    flex: 1;
  }

  .header-content {
    display: flex;
    align-items: center;
    gap: 12px;
    width: 100%;
  }

  .text-btn {
    background: none;
    border: none;
    color: var(--accent-primary);
    font-size: 11px;
    font-weight: 700;
    text-transform: uppercase;
    display: flex;
    align-items: center;
    gap: 6px;
    cursor: pointer;
    padding: 4px 8px;
    border-radius: 4px;
    transition: background 0.2s;
  }

  .text-btn:hover {
    background: rgba(231, 196, 107, 0.1);
  }

  .text-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .diag-metrics {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 20px;
  }

  .metric {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .metric span {
    font-size: 11px;
    color: var(--text-tertiary);
    text-transform: uppercase;
  }

  .metric strong {
    font-size: 15px;
    color: var(--text-primary);
  }

  .text-success { color: var(--accent-success) !important; }
  .text-warning { color: #f3c46b !important; }

  .tier-badge {
    display: inline-block;
    padding: 2px 8px;
    border-radius: 4px;
    background: rgba(231, 196, 107, 0.1);
    color: var(--accent-primary);
  }

  .model-list {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .model-item {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 16px;
    background: rgba(255,255,255,0.02);
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-md);
    transition: var(--transition-fast);
  }

  .model-item.busy {
    border-color: var(--accent-primary);
    background: rgba(231, 196, 107, 0.05);
  }

  .progress-container {
    margin-top: 8px;
    width: 200px;
    height: 4px;
    background: rgba(255,255,255,0.05);
    border-radius: 2px;
    position: relative;
    overflow: hidden;
  }

  .progress-bar {
    height: 100%;
    background: var(--accent-primary);
    box-shadow: 0 0 8px var(--accent-primary);
    transition: width 0.2s ease;
  }

  .model-info {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .m-type { font-size: 10px; text-transform: uppercase; color: var(--text-tertiary); }
  .m-name { font-size: 14px; font-weight: 600; color: var(--text-primary); }
  .m-size { font-size: 12px; color: var(--text-secondary); }

  .icon-btn {
    width: 32px;
    height: 32px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 50%;
    color: var(--text-secondary);
    transition: var(--transition-fast);
  }

  .icon-btn:hover { background: var(--bg-surface-elevated); color: var(--accent-primary); }

  .loading-state {
    padding: 20px;
    text-align: center;
    color: var(--text-tertiary);
    font-style: italic;
  }

  .analysis-progress {
    display: flex;
    flex-direction: column;
    gap: 4px;
    flex: 1;
    align-items: flex-end;
  }

  .status-text {
    font-size: 11px;
    color: var(--text-secondary);
  }

  .analysis-progress .progress-bar-bg {
    width: 100%;
    height: 4px;
    background: rgba(255,255,255,0.05);
    border-radius: 2px;
    overflow: hidden;
  }

  .analysis-progress .progress-bar-fill {
    height: 100%;
    background: var(--accent-primary);
    transition: width 0.2s ease;
  }

  :global(.spin) {
    animation: spin 1s linear infinite;
  }
  @keyframes spin {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
  }
</style>
