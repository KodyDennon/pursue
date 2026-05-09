<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import Logo from "$lib/components/Logo.svelte";

  export let onComplete: () => void;

  let step = 0;
  let statusText = "Verifying environment...";
  let progress = 0;
  
  onMount(async () => {
    // Mocking the model download sequence for now since the rust command isn't built yet
    // The real implementation would invoke a streaming Tauri command to download Gemma 4 and Vector models.
    setTimeout(() => {
      step = 1;
      statusText = "Downloading Gemma 4 Intelligence Engine...";
      let interval = setInterval(() => {
        progress += 5;
        if (progress >= 100) {
          clearInterval(interval);
          step = 2;
          progress = 0;
          statusText = "Downloading ONNX Embedding Models...";
          
          let int2 = setInterval(() => {
            progress += 10;
            if (progress >= 100) {
              clearInterval(int2);
              statusText = "Initializing Vector Database...";
              setTimeout(() => {
                onComplete();
              }, 1500);
            }
          }, 200);
        }
      }, 100);
    }, 1000);
  });
</script>

<div class="provision-screen">
  <div class="provision-card glass-panel">
    <Logo size={80} class="hero-logo" />
    <div class="brand-hero">PURSUE</div>
    <h2>Black-Ops Initialization</h2>
    <p class="status-mono mono">{statusText}</p>
    
    <div class="progress-bar-wrap">
      <div class="progress-fill" style="width: {progress}%"></div>
    </div>
    
    <div class="sys-reqs">
      <span>Gemma 4 OSINT</span>
      <span>2.1 GB</span>
    </div>
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
    margin-bottom: 32px;
    color: var(--text-primary);
  }
  
  .status-mono {
    font-size: 12px;
    color: var(--accent-success);
    margin-bottom: 16px;
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
</style>
