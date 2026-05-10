<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import Logo from "$lib/components/Logo.svelte";
  import { Cpu, Brain, HardDrive, ShieldAlert, CheckCircle, ChevronRight, Loader2 } from "lucide-svelte";

  let { onComplete } = $props<{ onComplete: () => void }>();

  let step = $state<"diagnostic" | "selection" | "provisioning">("diagnostic");
  let statusText = $state("Analyzing hardware environment...");
  let progress = $state(0);
  let specs = $state<any>(null);
  let missingModels = $state<Record<string, boolean>>({});
  let selectedTier = $state<"Standard" | "Elite">("Standard");

  const MODELS = {
    Standard: [
      { id: "bge-small", name: "BGE Small v1.5", filename: "bge-small-en-v1.5.onnx", url: "https://huggingface.co/BAAI/bge-small-en-v1.5/resolve/main/onnx/model.onnx" },
      { id: "tokenizer", name: "BGE Tokenizer", filename: "tokenizer.json", url: "https://huggingface.co/BAAI/bge-small-en-v1.5/resolve/main/tokenizer.json" },
      { id: "gemma-2b", name: "Gemma 4 2B IT", filename: "gemma-4-2b-it.gguf", url: "https://huggingface.co/google/gemma-4-2b-it-GGUF/resolve/main/gemma-4-2b-it.Q4_K_M.gguf" }
    ],
    Elite: [
      { id: "bge-small", name: "BGE Small v1.5", filename: "bge-small-en-v1.5.onnx", url: "https://huggingface.co/BAAI/bge-small-en-v1.5/resolve/main/onnx/model.onnx" },
      { id: "tokenizer", name: "BGE Tokenizer", filename: "tokenizer.json", url: "https://huggingface.co/BAAI/bge-small-en-v1.5/resolve/main/tokenizer.json" },
      { id: "gemma-4b", name: "Gemma 4 4B IT", filename: "gemma-4-4b-it.gguf", url: "https://huggingface.co/google/gemma-4-4b-it-GGUF/resolve/main/gemma-4-4b-it.Q4_K_M.gguf" }
    ]
  };

  onMount(async () => {
    // 1. Check Diagnostics
    specs = await invoke("get_hardware_diagnostics");
    missingModels = await invoke("check_model_status");
    
    selectedTier = specs.recommended_tier === 'Elite' ? 'Elite' : 'Standard';

    // Check if anything is actually missing
    const anyMissing = Object.values(missingModels).some(v => !v);
    if (!anyMissing) {
      onComplete();
      return;
    }

    setTimeout(() => {
      step = "selection";
      statusText = "Hardware Analysis Complete.";
    }, 1500);

    // Listen for progress
    const unlisten = await listen("model-progress", (event: any) => {
      const payload = event.payload;
      const allModels = [...MODELS.Standard, ...MODELS.Elite];
      const model = allModels.find(m => m.id === payload.model_id);
      
      if (payload.total_bytes) {
        progress = Math.round((payload.bytes_downloaded / payload.total_bytes) * 100);
      }
      statusText = `Downloading ${model?.name || 'Intelligence Asset'}...`;
    });

    return () => unlisten();
  });

  async function startProvisioning() {
    step = "provisioning";
    const modelsToDownload = MODELS[selectedTier];

    for (const model of modelsToDownload) {
      if (missingModels[model.id]) {
        progress = 100;
        continue;
      }
      
      try {
        await invoke("provision_model", { 
          id: model.id, 
          url: model.url, 
          name: model.filename 
        });
      } catch (e) {
        console.error(`Failed to download ${model.name}`, e);
      }
    }

    statusText = "Intelligence OS Initialized.";
    setTimeout(onComplete, 1000);
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
        <p>Recommended based on your <strong>{specs?.cpu_brand}</strong> and <strong>{specs?.total_memory_gb}GB RAM</strong>.</p>

        <div class="tier-options">
          <button 
            class="tier-card" 
            class:active={selectedTier === 'Standard'} 
            class:recommended={specs?.recommended_tier === 'Deep'}
            onclick={() => selectedTier = 'Standard'}
          >
            <div class="tier-head">
              <Cpu size={24} />
              <div class="t-title">Standard Intel</div>
            </div>
            <p>Gemma 2B + BGE Small. Balanced performance for mobile workstations.</p>
            <div class="tier-meta">1.8 GB Total</div>
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
            <p>Gemma 4B + BGE Small. Full forensic capabilities with deeper reasoning.</p>
            <div class="tier-meta">3.0 GB Total</div>
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
        <span>Gemma {selectedTier === 'Elite' ? '4B' : '2B'} IT</span>
        <span>{progress}%</span>
      </div>
    {/if}
  </div>
</div>

<style>
  .provision-screen {
    position: fixed;
    inset: 0;
    background: var(--bg-base);
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
  }
  
  :global(.hero-logo) {
    margin-bottom: 32px;
    filter: drop-shadow(0 0 20px rgba(34, 211, 238, 0.3));
  }
  
  .brand-hero {
    font-family: var(--font-display);
    font-weight: 700;
    font-size: 36px;
    letter-spacing: 0.2em;
    color: var(--accent-primary);
    margin-bottom: 24px;
  }
  
  h2 {
    font-size: 18px;
    margin-bottom: 8px;
    color: var(--text-primary);
  }

  p {
    font-size: 14px;
    color: var(--text-secondary);
    margin-bottom: 32px;
  }
  
  .status-mono {
    font-size: 12px;
    color: var(--accent-success);
    margin-bottom: 24px;
  }

  .diagnostic-loader {
    padding: 20px;
    color: var(--accent-primary);
  }
  
  .progress-bar-wrap {
    width: 100%;
    height: 4px;
    background: var(--bg-surface);
    border-radius: 2px;
    overflow: hidden;
    margin-bottom: 16px;
  }
  
  .progress-fill {
    height: 100%;
    background: var(--accent-primary);
    transition: width 0.2s ease-out;
  }
  
  .sys-reqs {
    display: flex;
    justify-content: space-between;
    width: 100%;
    font-size: 11px;
    color: var(--text-secondary);
    text-transform: uppercase;
    letter-spacing: 0.1em;
  }

  .tier-options {
    display: flex;
    gap: 16px;
    margin-bottom: 40px;
    width: 100%;
  }

  .tier-card {
    flex: 1;
    background: rgba(255,255,255,0.02);
    border: 1px solid var(--border-subtle);
    border-radius: 12px;
    padding: 24px;
    text-align: left;
    cursor: pointer;
    transition: all 0.2s;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .tier-card p {
    font-size: 12px;
    margin: 0;
    line-height: 1.5;
  }

  .tier-card.active {
    border-color: var(--accent-primary);
    background: rgba(231, 196, 107, 0.05);
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
  }

  .provision-btn {
    background: var(--accent-primary);
    color: #000;
    border: none;
    border-radius: 8px;
    padding: 12px 24px;
    font-weight: 700;
    font-size: 15px;
    display: flex;
    align-items: center;
    gap: 12px;
    cursor: pointer;
    transition: transform 0.2s;
  }

  .provision-btn:hover {
    transform: translateY(-2px);
  }

  .provision-card.selection {
    width: 680px;
  }

  :global(.spin) {
    animation: spin 1s linear infinite;
  }
  @keyframes spin {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
  }
</style>
