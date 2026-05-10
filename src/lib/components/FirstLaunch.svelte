<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import Logo from "$lib/components/Logo.svelte";
  import { Cpu, Brain, HardDrive, ShieldAlert, CheckCircle, ChevronRight, Loader2, Download } from "lucide-svelte";

  let { onComplete } = $props<{ onComplete: () => void }>();

  let step = $state<"diagnostic" | "selection" | "provisioning" | "ready">("diagnostic");
  let statusText = $state("Analyzing hardware environment...");
  let progress = $state(0);
  let specs = $state<any>(null);
  let modelStatus = $state<Record<string, boolean>>({});
  let selectedTier = $state<"Standard" | "Elite">("Standard");
  let currentModelName = $state("");

  const MODELS = {
    Standard: [
      { id: "bge-small", name: "BGE Small v1.5", filename: "bge-small-en-v1.5.onnx", url: "https://huggingface.co/BAAI/bge-small-en-v1.5/resolve/main/onnx/model.onnx" },
      { id: "tokenizer", name: "BGE Tokenizer", filename: "tokenizer.json", url: "https://huggingface.co/BAAI/bge-small-en-v1.5/resolve/main/tokenizer.json" },
      { id: "gemma-4-e2b", name: "Gemma 4 E2B IT", filename: "gemma-4-e2b-it.gguf", url: "https://huggingface.co/google/gemma-4-E2B-it-GGUF/resolve/main/gemma-4-e2b-it.Q4_K_M.gguf" }
    ],
    Elite: [
      { id: "bge-small", name: "BGE Small v1.5", filename: "bge-small-en-v1.5.onnx", url: "https://huggingface.co/BAAI/bge-small-en-v1.5/resolve/main/onnx/model.onnx" },
      { id: "tokenizer", name: "BGE Tokenizer", filename: "tokenizer.json", url: "https://huggingface.co/BAAI/bge-small-en-v1.5/resolve/main/tokenizer.json" },
      { id: "gemma-4-e4b", name: "Gemma 4 E4B IT", filename: "gemma-4-e4b-it.gguf", url: "https://huggingface.co/google/gemma-4-E4B-it-GGUF/resolve/main/gemma-4-e4b-it.Q4_K_M.gguf" }
    ]
  };

  onMount(() => {
    // 1. Check Diagnostics and Provisioning status
    (async () => {
      try {
        specs = await invoke("get_hardware_diagnostics");
        modelStatus = await invoke("check_model_status");
        
        selectedTier = specs.recommended_tier === 'Elite' ? 'Elite' : 'Standard';

        const requiredIds = MODELS[selectedTier].map(m => m.id);
        const allPresent = requiredIds.every(id => modelStatus[id]);
        
        if (allPresent) {
            step = "ready";
            statusText = "Intelligence OS already provisioned.";
            setTimeout(onComplete, 500);
            return;
        }

        step = "selection";
        statusText = "Environment scan complete.";
      } catch (e) {
        console.error("Initialization probe failed", e);
        statusText = "Hardware probe failed. Using standard profile.";
        step = "selection";
      }
    })();

    // Listen for progress
    let unlisten: (() => void) | undefined;
    listen("model-progress", (event: any) => {
      const payload = event.payload;
      if (payload.total_bytes) {
        progress = Math.round((payload.bytes_downloaded / payload.total_bytes) * 100);
      }
    }).then(u => unlisten = u);

    return () => {
      if (unlisten) unlisten();
    };
  });

  async function startProvisioning() {
    step = "provisioning";
    const modelsToDownload = MODELS[selectedTier];

    for (const model of modelsToDownload) {
      currentModelName = model.name;
      progress = 0;
      
      // Re-check status just in case
      const currentStatus = await invoke<Record<string, boolean>>("check_model_status");
      if (currentStatus[model.id]) {
        progress = 100;
        continue;
      }
      
      try {
        statusText = `Provisioning ${model.name}...`;
        await invoke("provision_model", { 
          id: model.id, 
          url: model.url, 
          name: model.filename 
        });
      } catch (e) {
        console.error(`Failed to download ${model.name}`, e);
        statusText = `Error provisioning ${model.name}. Retrying...`;
        await new Promise(r => setTimeout(r, 2000));
        // Try again once
        try {
           await invoke("provision_model", { id: model.id, url: model.url, name: model.filename });
        } catch(e2) {
           console.error("Critical download failure", e2);
        }
      }
    }

    step = "ready";
    statusText = "Intelligence OS Initialized.";
    setTimeout(onComplete, 1500);
  }
</script>

<div class="provision-screen">
  <div class="provision-card glass-panel" class:selection={step === 'selection'}>
    <Logo size={60} class="hero-logo" />
    <div class="brand-hero">PURSUE</div>
    
    {#if step === 'diagnostic'}
      <h2>Black-Ops Initialization</h2>
      <p class="status-mono mono">{statusText}</p>
      <div class="diagnostic-loader">
        <Loader2 size={24} class="spin" />
      </div>
    {:else if step === 'selection'}
      <div class="selection-view">
        <h2>Intelligence Tier Selection</h2>
        <p>Recommended based on your <strong>{specs?.cpu_brand || 'Processor'}</strong> and <strong>{specs?.total_memory_gb || '??'}GB RAM</strong>.</p>

        <div class="tier-options">
          <button 
            class="tier-card" 
            class:active={selectedTier === 'Standard'} 
            class:recommended={specs?.recommended_tier === 'Standard' || specs?.recommended_tier === 'Deep'}
            onclick={() => selectedTier = 'Standard'}
          >
            <div class="tier-head">
              <Cpu size={24} />
              <div class="t-title">Standard Intel</div>
            </div>
            <p>Gemma 4 E2B + BGE. Optimized effective parameter architecture for workstation performance.</p>
            <div class="tier-meta">~3.2 GB Storage</div>
          </button>

          <button 
            class="tier-card" 
            class:active={selectedTier === 'Elite'} 
            class:recommended={specs?.recommended_tier === 'Elite'}
            onclick={() => selectedTier = 'Elite'}
          >
            <div class="tier-head">
              <Brain size={24} />
              <div class="t-title">Elite Intel</div>
            </div>
            <p>Gemma 4 E4B + BGE. Advanced reasoning with native multimodal capabilities.</p>
            <div class="tier-meta">~5.0 GB Storage</div>
          </button>
        </div>

        <button class="provision-btn" onclick={startProvisioning}>
          Initialize Neural OS <ChevronRight size={18} />
        </button>
      </div>
    {:else if step === 'provisioning'}
      <h2>Provisioning Intelligence Engine</h2>
      <p class="status-mono mono">{statusText}</p>
      
      <div class="progress-bar-wrap">
        <div class="progress-fill" style="width: {progress}%"></div>
      </div>
      
      <div class="sys-reqs">
        <span>{currentModelName}</span>
        <span>{progress}%</span>
      </div>
    {:else if step === 'ready'}
      <h2>Systems Ready</h2>
      <p class="status-mono mono">{statusText}</p>
      <div class="ready-check">
        <CheckCircle size={48} class="accent-success" />
      </div>
    {/if}
  </div>
</div>

<style>
  .provision-screen {
    position: fixed;
    inset: 0;
    background: #050608;
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 9999;
  }
  
  .provision-card {
    width: 480px;
    padding: 40px;
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    box-shadow: 0 0 50px rgba(0,0,0,0.5);
  }
  
  :global(.hero-logo) {
    margin-bottom: 32px;
    filter: drop-shadow(0 0 20px rgba(231, 196, 107, 0.3));
  }
  
  .brand-hero {
    font-family: var(--font-display);
    font-weight: 700;
    font-size: 36px;
    letter-spacing: 0.25em;
    color: var(--accent-primary);
    margin-bottom: 24px;
    text-shadow: 0 0 10px rgba(231, 196, 107, 0.2);
  }
  
  h2 {
    font-size: 20px;
    margin-bottom: 8px;
    color: var(--text-primary);
    letter-spacing: 0.05em;
  }

  p {
    font-size: 14px;
    color: var(--text-secondary);
    margin-bottom: 32px;
  }
  
  .status-mono {
    font-size: 11px;
    color: var(--accent-success);
    margin-bottom: 24px;
    text-transform: uppercase;
    letter-spacing: 0.1em;
  }

  .diagnostic-loader {
    padding: 20px;
    color: var(--accent-primary);
  }
  
  .progress-bar-wrap {
    width: 100%;
    height: 6px;
    background: rgba(255,255,255,0.05);
    border-radius: 3px;
    overflow: hidden;
    margin-bottom: 16px;
    border: 1px solid rgba(255,255,255,0.02);
  }
  
  .progress-fill {
    height: 100%;
    background: var(--accent-primary);
    box-shadow: 0 0 15px var(--accent-primary);
    transition: width 0.3s cubic-bezier(0.4, 0, 0.2, 1);
  }
  
  .sys-reqs {
    display: flex;
    justify-content: space-between;
    width: 100%;
    font-size: 10px;
    color: var(--text-tertiary);
    text-transform: uppercase;
    letter-spacing: 0.1em;
    font-weight: 700;
  }

  .tier-options {
    display: flex;
    gap: 16px;
    margin-bottom: 40px;
    width: 100%;
  }

  .tier-card {
    flex: 1;
    background: rgba(255,255,255,0.01);
    border: 1px solid var(--border-subtle);
    border-radius: 12px;
    padding: 24px;
    text-align: left;
    cursor: pointer;
    transition: all 0.3s;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .tier-card p {
    font-size: 12px;
    margin: 0;
    line-height: 1.5;
    color: var(--text-tertiary);
  }

  .tier-card.active {
    border-color: var(--accent-primary);
    background: rgba(231, 196, 107, 0.05);
    box-shadow: 0 0 20px rgba(231, 196, 107, 0.1);
  }

  .tier-card.recommended {
    position: relative;
  }
  .tier-card.recommended::before {
    content: 'RECOMMENDED';
    position: absolute;
    top: -10px;
    right: 12px;
    font-size: 8px;
    background: var(--accent-primary);
    color: #000;
    padding: 2px 6px;
    border-radius: 4px;
    font-weight: 800;
  }

  .tier-head {
    display: flex;
    align-items: center;
    gap: 12px;
    color: var(--accent-primary);
  }

  .t-title {
    font-weight: 700;
    font-size: 15px;
    color: var(--text-primary);
  }

  .tier-meta {
    font-size: 10px;
    color: var(--text-tertiary);
    text-transform: uppercase;
    margin-top: auto;
  }

  .provision-btn {
    width: 100%;
    background: var(--accent-primary);
    color: #000;
    border: none;
    border-radius: 8px;
    padding: 16px;
    font-weight: 800;
    font-size: 15px;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 12px;
    cursor: pointer;
    transition: all 0.3s;
    text-transform: uppercase;
    letter-spacing: 0.1em;
  }

  .provision-btn:hover {
    filter: brightness(1.1);
    transform: translateY(-2px);
  }

  .provision-card.selection {
    width: 720px;
  }

  .ready-check {
    margin-top: 20px;
    color: var(--accent-success);
    animation: scale-in 0.5s cubic-bezier(0.175, 0.885, 0.32, 1.275);
  }

  @keyframes scale-in {
    from { transform: scale(0); opacity: 0; }
    to { transform: scale(1); opacity: 1; }
  }

  :global(.spin) {
    animation: spin 1s linear infinite;
  }
  @keyframes spin {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
  }
</style>
