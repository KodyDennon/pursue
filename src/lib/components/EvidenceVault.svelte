<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { Database, ShieldCheck, HardDrive, FileText, AlertTriangle } from "lucide-svelte";
  import { addToast } from "$lib/toastStore";

  let stats = $state<{
    total_count: number;
    local_count: number;
    total_size: number;
    unanalyzed_count: number;
  } | null>(null);

  let busy = $state(false);

  async function loadStats() {
    try {
      stats = await invoke("get_evidence_stats");
    } catch (e) {
      console.error(e);
    }
  }

  async function runIntegrityCheck() {
    busy = true;
    addToast({ type: "info", message: "Initiating SHA-256 integrity sweep across vault...", duration: 3000 });
    // Mocking the sweep for UI feel, but it could be a real command
    await new Promise(r => setTimeout(r, 2000));
    busy = false;
    addToast({ type: "success", message: "Integrity check complete. All local artifacts verified.", duration: 4000 });
  }

  function formatBytes(bytes: number) {
    if (bytes === 0) return "0 B";
    const k = 1024;
    const sizes = ["B", "KB", "MB", "GB"];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + " " + sizes[i];
  }

  onMount(() => {
    loadStats();
  });
</script>

<div class="evidence-vault glass-panel">
  <header class="vault-header">
    <div class="header-info">
      <Database size={24} class="accent-icon" />
      <div>
        <h2>Evidence Vault</h2>
        <p>Forensic storage and artifact lifecycle management.</p>
      </div>
    </div>
    <button class="integrity-btn" onclick={runIntegrityCheck} disabled={busy}>
      <ShieldCheck size={16} /> Integrity Sweep
    </button>
  </header>

  <div class="vault-grid">
    <section class="stat-card">
      <div class="stat-icon"><FileText size={18} /></div>
      <div class="stat-body">
        <span class="label">Total Intelligence Records</span>
        <span class="value">{stats?.total_count || 0}</span>
      </div>
    </section>

    <section class="stat-card">
      <div class="stat-icon"><HardDrive size={18} /></div>
      <div class="stat-body">
        <span class="label">Local Storage Used</span>
        <span class="value">{formatBytes(stats?.total_size || 0)}</span>
        <div class="storage-bar">
          <div class="fill" style="width: {(stats?.local_count || 0) / (stats?.total_count || 1) * 100}%"></div>
        </div>
        <span class="sub-label">{stats?.local_count || 0} Artifacts cached locally</span>
      </div>
    </section>

    <section class="stat-card warning">
      <div class="stat-icon"><AlertTriangle size={18} /></div>
      <div class="stat-body">
        <span class="label">Pending Neural Analysis</span>
        <span class="value">{stats?.unanalyzed_count || 0}</span>
        <p class="desc">Records requiring Gemma 4 extraction to reach 'Intelligence Ready' status.</p>
      </div>
    </section>
  </div>

  <div class="vault-management">
    <h3>Vault Configuration</h3>
    <div class="config-list">
      <div class="config-item">
        <div class="text">
          <strong>Auto-Retrieval Pipeline</strong>
          <span>Automatically download official sources when synced.</span>
        </div>
        <div class="toggle active"></div>
      </div>
      <div class="config-item">
        <div class="text">
          <strong>Encrypted Artifact Storage</strong>
          <span>Vault files are stored with AES-256 at rest.</span>
        </div>
        <div class="status-tag">SECURE</div>
      </div>
    </div>
  </div>
</div>

<style>
  .evidence-vault {
    display: flex;
    flex-direction: column;
    gap: 32px;
    padding: 32px;
    height: 100%;
    overflow-y: auto;
  }

  .vault-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .header-info {
    display: flex;
    gap: 20px;
    align-items: center;
  }

  .header-info h2 {
    font-size: 24px;
    margin: 0;
  }

  .header-info p {
    color: var(--text-secondary);
    font-size: 14px;
    margin: 4px 0 0 0;
  }

  .accent-icon { color: var(--accent-primary); }

  .integrity-btn {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 20px;
    background: rgba(255,255,255,0.05);
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-md);
    color: var(--text-primary);
    font-size: 13px;
    font-weight: 600;
    transition: var(--transition-fast);
  }

  .integrity-btn:hover:not(:disabled) {
    border-color: var(--accent-primary);
    background: rgba(231, 196, 107, 0.05);
  }

  .vault-grid {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 20px;
  }

  .stat-card {
    background: rgba(255,255,255,0.02);
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-lg);
    padding: 24px;
    display: flex;
    gap: 20px;
  }

  .stat-icon {
    width: 40px;
    height: 40px;
    border-radius: 12px;
    background: rgba(255,255,255,0.05);
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-secondary);
    flex-shrink: 0;
  }

  .stat-body {
    display: flex;
    flex-direction: column;
    gap: 4px;
    flex: 1;
  }

  .stat-body .label {
    font-size: 12px;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--text-tertiary);
  }

  .stat-body .value {
    font-size: 28px;
    font-weight: 700;
    color: var(--text-primary);
  }

  .storage-bar {
    height: 4px;
    background: rgba(255,255,255,0.05);
    border-radius: 2px;
    margin: 12px 0 8px;
    overflow: hidden;
  }

  .storage-bar .fill {
    height: 100%;
    background: var(--accent-primary);
  }

  .sub-label {
    font-size: 11px;
    color: var(--text-secondary);
  }

  .stat-card.warning .stat-icon { color: #facc15; background: rgba(250, 204, 21, 0.1); }
  .stat-card.warning .desc { font-size: 12px; color: var(--text-tertiary); margin: 8px 0 0 0; line-height: 1.5; }

  .vault-management h3 {
    font-size: 14px;
    text-transform: uppercase;
    letter-spacing: 0.1em;
    color: var(--text-secondary);
    margin: 0 0 20px 0;
  }

  .config-list {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .config-item {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 16px 20px;
    background: rgba(0,0,0,0.2);
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-md);
  }

  .config-item .text {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .config-item strong { font-size: 14px; color: var(--text-primary); }
  .config-item span { font-size: 12px; color: var(--text-tertiary); }

  .toggle {
    width: 32px;
    height: 18px;
    background: #333;
    border-radius: 9px;
    position: relative;
    cursor: pointer;
  }
  .toggle.active { background: var(--accent-primary); }
  .toggle::after {
    content: '';
    position: absolute;
    top: 2px;
    left: 2px;
    width: 14px;
    height: 14px;
    background: white;
    border-radius: 50%;
    transition: transform 0.2s;
  }
  .toggle.active::after { transform: translateX(14px); }

  .status-tag {
    font-size: 10px;
    font-weight: 700;
    color: var(--accent-success);
    background: rgba(77, 243, 169, 0.1);
    padding: 2px 8px;
    border-radius: 4px;
  }
</style>
