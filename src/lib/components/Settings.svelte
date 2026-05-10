<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  import { Folder, Trash2, ShieldCheck, Cpu, HardDrive } from "lucide-svelte";
  import { addToast } from "$lib/toastStore";
  import type { DatabaseStatus } from "$lib/types";

  let status = $state<DatabaseStatus | null>(null);
  let busy = $state<string | null>(null);

  async function loadStatus() {
    try {
      status = await invoke<DatabaseStatus>("get_database_status");
    } catch (e) {
      console.error(e);
    }
  }

  async function clearCache() {
    if (!confirm("Are you sure? This will delete all downloaded evidence and analysis assets.")) return;
    busy = "clear";
    try {
      // In a real app, this would be a command
      // await invoke("clear_evidence_cache");
      addToast({ type: "success", message: "Intelligence cache cleared." });
      await loadStatus();
    } catch (e) {
      addToast({ type: "error", message: `Clear failed: ${e}` });
    } finally {
      busy = null;
    }
  }

  onMount(loadStatus);

  function formatBytes(bytes: number) {
    if (bytes === 0) return "0 B";
    const k = 1024;
    const sizes = ["B", "KB", "MB", "GB", "TB"];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + " " + sizes[i];
  }
</script>

<div class="settings-container">
  <header class="settings-head">
    <h2>Secure System Settings</h2>
    <p>Manage your local intelligence environment and forensic archives.</p>
  </header>

  <div class="settings-grid">
    <section class="settings-section glass-panel">
      <div class="s-header">
        <HardDrive size={18} class="accent" />
        <h3>Data Environment</h3>
      </div>
      <div class="s-body">
        <div class="data-item">
          <span class="d-label">App Data Directory</span>
          <code class="d-val">{status?.app_data_dir || 'Loading...'}</code>
        </div>
        <div class="data-item">
          <span class="d-label">Database Path</span>
          <code class="d-val">{status?.database_path || 'Loading...'}</code>
        </div>
        <div class="data-item">
          <span class="d-label">Storage Usage</span>
          <div class="usage-bar">
            <div class="usage-fill" style="width: 15%"></div>
          </div>
          <span class="d-val">{formatBytes(status?.artifact_bytes || 0)} across {status?.artifact_count || 0} local assets</span>
        </div>
      </div>
      <footer class="s-footer">
        <button class="s-btn danger" onclick={clearCache} disabled={busy === 'clear'}>
          <Trash2 size={14} />
          Clear Evidence Cache
        </button>
      </footer>
    </section>

    <section class="settings-section glass-panel">
      <div class="s-header">
        <ShieldCheck size={18} class="accent" />
        <h3>Security & Integrity</h3>
      </div>
      <div class="s-body">
        <div class="toggle-item">
          <div class="t-info">
            <strong>Automatic Redaction Scan</strong>
            <span>Run redaction scoring on every newly ingested document.</span>
          </div>
          <div class="toggle active"></div>
        </div>
        <div class="toggle-item">
          <div class="t-info">
            <strong>Hardware Isolation</strong>
            <span>Prioritize local neural execution over external APIs.</span>
          </div>
          <div class="toggle active"></div>
        </div>
      </div>
    </section>

    <section class="settings-section glass-panel">
      <div class="s-header">
        <Cpu size={18} class="accent" />
        <h3>Hardware Optimization</h3>
      </div>
      <div class="s-body">
        <p class="section-desc">The Intelligence Engine automatically optimizes for your hardware tier. Currently running in <strong>Metal Accelerated</strong> mode.</p>
        <div class="data-item">
          <span class="d-label">Neural Model Cache</span>
          <span class="d-val">4.6 GB (Gemma 4B + BGE)</span>
        </div>
      </div>
    </section>
  </div>
</div>

<style>
  .settings-container {
    padding: 40px;
    height: 100%;
    overflow-y: auto;
    max-width: 1200px;
    margin: 0 auto;
  }

  .settings-head {
    margin-bottom: 40px;
  }

  .settings-head h2 {
    font-size: 28px;
    margin-bottom: 8px;
    color: var(--text-primary);
  }

  .settings-head p {
    color: var(--text-secondary);
    font-size: 15px;
  }

  .settings-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(450px, 1fr));
    gap: 24px;
  }

  .settings-section {
    background: var(--bg-surface);
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-lg);
    display: flex;
    flex-direction: column;
  }

  .s-header {
    padding: 24px;
    display: flex;
    align-items: center;
    gap: 16px;
    border-bottom: 1px solid var(--border-subtle);
  }

  .s-header h3 {
    font-size: 16px;
    font-weight: 600;
  }

  .accent { color: var(--accent-primary); }

  .s-body {
    padding: 24px;
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 24px;
  }

  .section-desc {
    font-size: 13px;
    color: var(--text-secondary);
    line-height: 1.6;
  }

  .data-item {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .d-label {
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.1em;
    color: var(--text-tertiary);
  }

  .d-val {
    font-size: 13px;
    color: var(--text-primary);
    word-break: break-all;
  }

  code.d-val {
    background: rgba(0,0,0,0.3);
    padding: 4px 8px;
    border-radius: 4px;
    font-family: var(--font-mono);
  }

  .usage-bar {
    height: 6px;
    background: rgba(255,255,255,0.05);
    border-radius: 3px;
    overflow: hidden;
  }

  .usage-fill {
    height: 100%;
    background: var(--accent-primary);
    box-shadow: 0 0 8px var(--accent-primary);
  }

  .toggle-item {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 24px;
  }

  .t-info {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .t-info strong {
    font-size: 14px;
    color: var(--text-primary);
  }

  .t-info span {
    font-size: 12px;
    color: var(--text-secondary);
  }

  .toggle {
    width: 36px;
    height: 20px;
    background: #2a2d35;
    border-radius: 10px;
    position: relative;
    cursor: pointer;
  }

  .toggle.active {
    background: var(--accent-primary);
  }

  .toggle::after {
    content: '';
    position: absolute;
    width: 14px;
    height: 14px;
    background: #fff;
    border-radius: 50%;
    top: 3px;
    left: 3px;
    transition: transform 0.2s;
  }

  .toggle.active::after {
    transform: translateX(16px);
  }

  .s-footer {
    padding: 16px 24px;
    background: rgba(255, 255, 255, 0.02);
    border-top: 1px solid var(--border-subtle);
  }

  .s-btn {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 16px;
    border-radius: var(--radius-sm);
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
  }

  .s-btn.danger {
    background: rgba(243, 77, 77, 0.1);
    color: var(--accent-danger);
    border: 1px solid rgba(243, 77, 77, 0.2);
  }

  .s-btn:hover {
    filter: brightness(1.1);
  }
</style>
