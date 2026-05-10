<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onMount, onDestroy } from "svelte";
  import { Loader2, Download, CheckCircle2, AlertCircle, XCircle } from "lucide-svelte";
  import type { BulkDownloadReport, BulkDownloadItem, BulkDownloadStatus } from "$lib/types";
  import { addToast } from "$lib/toastStore";

  let { onComplete } = $props<{ onComplete?: () => void }>();

  let activeJobId = $state<string | null>(null);
  let report = $state<BulkDownloadReport | null>(null);
  let polling = $state(false);
  let pollInterval: any = null;

  async function startBulkDownload() {
    try {
      activeJobId = await invoke<string>("download_missing_records");
      startPolling();
      addToast({ type: "info", message: "Intelligence Agent initiated bulk collection.", duration: 3000 });
    } catch (e) {
      addToast({ type: "error", message: `Agent failed: ${e}` });
    }
  }

  async function cancelDownload() {
    if (!activeJobId) return;
    try {
      await invoke("cancel_bulk_download", { id: activeJobId });
    } catch (e) {
      console.error(e);
    }
  }

  async function fetchStatus() {
    if (!activeJobId) return;
    try {
      report = await invoke<BulkDownloadReport>("get_bulk_download_status", { id: activeJobId });
      if (report.job.status === "completed" || report.job.status === "failed" || report.job.status === "cancelled") {
        stopPolling();
        if (onComplete) onComplete();
      }
    } catch (e) {
      console.error("Poll failed", e);
      stopPolling();
    }
  }

  function startPolling() {
    if (polling) return;
    polling = true;
    fetchStatus();
    pollInterval = setInterval(fetchStatus, 2000);
  }

  function stopPolling() {
    polling = false;
    if (pollInterval) clearInterval(pollInterval);
  }

  onDestroy(() => stopPolling());

  function getProgress(job: BulkDownloadStatus) {
    if (job.total === 0) return 0;
    return ((job.completed + job.failed) / (job.total - job.skipped)) * 100;
  }

  function formatBytes(bytes: number) {
    if (bytes === 0) return "0 B";
    const k = 1024;
    const sizes = ["B", "KB", "MB", "GB"];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + " " + sizes[i];
  }
</script>

<div class="agent-container glass-panel">
  <div class="agent-header">
    <div class="agent-info">
      <Download size={20} class="accent-icon" />
      <div class="text">
        <h3>Intelligence Collection Agent</h3>
        <p>Automated retrieval of official source documentation and media assets.</p>
      </div>
    </div>
    
    {#if !activeJobId}
      <button class="agent-btn primary" onclick={startBulkDownload}>
        Start Global Sync
      </button>
    {:else if report?.job.status === 'running' || report?.job.status === 'queued'}
      <button class="agent-btn danger" onclick={cancelDownload}>
        Terminate Sync
      </button>
    {/if}
  </div>

  {#if report}
    <div class="agent-progress">
      <div class="progress-stats">
        <span class="status-badge {report.job.status}">
          {report.job.status.replace('_', ' ')}
        </span>
        <span class="count">
          {report.job.completed + report.job.failed} / {report.job.total - report.job.skipped} Assets
        </span>
      </div>
      
      <div class="progress-bar-bg">
        <div class="progress-bar-fill" style="width: {getProgress(report.job)}%"></div>
      </div>

      <div class="mini-stats">
        <span>Completed: <strong>{report.job.completed}</strong></span>
        <span>Failed: <strong class={report.job.failed > 0 ? 'text-error' : ''}>{report.job.failed}</strong></span>
        <span>Skipped (Cached): <strong>{report.job.skipped}</strong></span>
      </div>
    </div>

    <div class="asset-list custom-scrollbar">
      {#each report.items as item}
        <div class="asset-item {item.status}">
          <div class="asset-icon">
            {#if item.status === 'completed'}
              <CheckCircle2 size={14} class="text-success" />
            {:else if item.status === 'failed'}
              <AlertCircle size={14} class="text-error" />
            {:else if item.status === 'downloading'}
              <Loader2 size={14} class="spin text-accent" />
            {:else}
              <div class="dot"></div>
            {/if}
          </div>
          <div class="asset-details">
            <span class="asset-title">{item.title}</span>
            <span class="asset-meta">
              {#if item.status === 'completed'}
                {formatBytes(item.bytes_downloaded)} • Verified
              {:else if item.status === 'failed'}
                Error: {item.error || 'Unknown failure'}
              {:else}
                {item.status}...
              {/if}
            </span>
          </div>
        </div>
      {/each}
    </div>
  {:else}
    <div class="agent-idle">
      <p>Agent is currently standby. Monitoring 573 official records.</p>
    </div>
  {/if}
</div>

<style>
  .agent-container {
    display: flex;
    flex-direction: column;
    gap: 20px;
    padding: 24px;
    border-radius: var(--radius-lg);
    background: var(--bg-surface);
    border: 1px solid var(--border-subtle);
    height: 100%;
  }

  .agent-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .agent-info {
    display: flex;
    gap: 16px;
    align-items: center;
  }

  .agent-info h3 {
    font-size: 16px;
    font-weight: 600;
    margin: 0;
    color: var(--text-primary);
  }

  .agent-info p {
    font-size: 13px;
    color: var(--text-secondary);
    margin: 4px 0 0 0;
  }

  .accent-icon { color: var(--accent-primary); }

  .agent-btn {
    padding: 8px 18px;
    border-radius: var(--radius-sm);
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
    transition: var(--transition-fast);
  }

  .agent-btn.primary {
    background: var(--accent-primary);
    color: #000;
    border: none;
  }

  .agent-btn.danger {
    background: rgba(255, 70, 70, 0.1);
    color: #ff4646;
    border: 1px solid rgba(255, 70, 70, 0.3);
  }

  .agent-btn:hover { filter: brightness(1.1); }

  .agent-progress {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .progress-stats {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .status-badge {
    font-size: 11px;
    text-transform: uppercase;
    font-weight: 700;
    padding: 2px 8px;
    border-radius: 4px;
    letter-spacing: 0.05em;
  }

  .status-badge.running { background: rgba(50, 150, 255, 0.2); color: #3296ff; }
  .status-badge.completed { background: rgba(0, 200, 100, 0.2); color: #00c864; }
  .status-badge.failed { background: rgba(255, 70, 70, 0.2); color: #ff4646; }

  .count { font-size: 12px; color: var(--text-secondary); }

  .progress-bar-bg {
    height: 6px;
    background: rgba(255,255,255,0.05);
    border-radius: 3px;
    overflow: hidden;
  }

  .progress-bar-fill {
    height: 100%;
    background: var(--accent-primary);
    transition: width 0.4s ease;
    box-shadow: 0 0 10px var(--accent-primary);
  }

  .mini-stats {
    display: flex;
    gap: 16px;
    font-size: 11px;
    color: var(--text-secondary);
  }

  .mini-stats strong { color: var(--text-primary); }
  .text-error { color: #ff4646 !important; }

  .asset-list {
    flex: 1;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding-right: 8px;
  }

  .asset-item {
    display: flex;
    gap: 12px;
    padding: 10px;
    border-radius: var(--radius-sm);
    background: rgba(255,255,255,0.02);
    border: 1px solid transparent;
    transition: var(--transition-fast);
  }

  .asset-item:hover { background: rgba(255,255,255,0.04); }

  .asset-icon {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 16px;
  }

  .dot { width: 4px; height: 4px; border-radius: 50%; background: var(--text-tertiary); }

  .asset-details {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .asset-title {
    font-size: 13px;
    color: var(--text-primary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 400px;
  }

  .asset-meta {
    font-size: 11px;
    color: var(--text-tertiary);
  }

  .text-success { color: #00c864; }
  .text-accent { color: var(--accent-primary); }

  .agent-idle {
    padding: 40px;
    text-align: center;
    color: var(--text-tertiary);
    font-size: 13px;
    border: 1px dashed var(--border-subtle);
    border-radius: var(--radius-md);
  }

  :global(.spin) {
    animation: spin 1s linear infinite;
  }
  @keyframes spin {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
  }
</style>
