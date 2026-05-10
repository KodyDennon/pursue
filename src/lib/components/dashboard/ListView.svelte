<script lang="ts">
  import type { RecordSummary } from "$lib/types";
  import { FileText, CheckCircle2, Clock, Download, ExternalLink, Zap, Maximize2 } from "lucide-svelte";

  let { records, selectedRecordId = null, onSelect, onView } = $props<{
    records: RecordSummary[];
    selectedRecordId?: string | null;
    onSelect: (record: RecordSummary) => void;
    onView?: (record: RecordSummary) => void;
  }>();

  function formatBytes(value: number | null | undefined) {
    if (!value) return "0 B";
    const units = ["B", "KB", "MB", "GB", "TB"];
    let next = value;
    let unit = 0;
    while (next >= 1024 && unit < units.length - 1) {
      next /= 1024;
      unit += 1;
    }
    return `${next.toFixed(next >= 10 || unit === 0 ? 0 : 1)} ${units[unit]}`;
  }
</script>

<div class="list-view custom-scrollbar">
  <table class="intel-table">
    <thead>
      <tr>
        <th class="col-status">Status</th>
        <th class="col-title">Record Title</th>
        <th class="col-agency">Agency</th>
        <th class="col-date">Released</th>
        <th class="col-size">Size</th>
        <th class="col-actions">Source</th>
      </tr>
    </thead>
    <tbody>
      {#each records as record}
        <tr 
          class:selected={selectedRecordId === record.id} 
          onclick={() => onSelect(record)}
        >
          <td class="col-status">
            <div class="status-indicator" class:ready={record.analysis_status === 'completed'} class:indexed={record.analysis_status === 'indexed'}>
              {#if record.analysis_status === 'completed'}
                <CheckCircle2 size={14} />
              {:else if record.analysis_status === 'indexed'}
                <Zap size={14} />
              {:else}
                <Clock size={14} />
              {/if}
            </div>
          </td>
          <td class="col-title">
            <div class="title-cell">
              <FileText size={14} class="type-icon" />
              <span>{record.title}</span>
            </div>
          </td>
          <td class="col-agency">
            <span class="badge agency-badge">{record.agency || "N/A"}</span>
          </td>
          <td class="col-date">{record.release_date || "--"}</td>
          <td class="col-size">{record.local_path ? formatBytes(record.artifact_size) : "--"}</td>
          <td class="col-actions">
            <div class="row-actions">
              {#if record.document_url}
                <a href={record.document_url} target="_blank" class="source-link" onclick={(e) => e.stopPropagation()} title="Open Remote Source">
                  <ExternalLink size={14} />
                </a>
              {/if}
              {#if record.local_path && onView}
                <button class="preview-link" onclick={(e) => { e.stopPropagation(); onView(record); }} title="Quick Preview">
                  <Maximize2 size={14} />
                </button>
              {/if}
            </div>
          </td>
        </tr>
      {/each}
    </tbody>
  </table>
  
  {#if records.length === 0}
    <div class="empty-state">No records found in current collection.</div>
  {/if}
</div>

<style>
  .list-view {
    height: 100%;
    overflow-y: auto;
    background: var(--bg-base);
  }

  .intel-table {
    width: 100%;
    border-collapse: collapse;
    font-size: 13px;
  }

  thead {
    position: sticky;
    top: 0;
    z-index: 10;
    background: var(--bg-surface-elevated);
    box-shadow: 0 1px 0 var(--border-subtle);
  }

  th {
    text-align: left;
    padding: 12px 16px;
    color: var(--text-tertiary);
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    font-size: 11px;
  }

  tr {
    border-bottom: 1px solid var(--border-subtle);
    cursor: pointer;
    transition: background 0.1s;
  }

  tr:hover {
    background: rgba(255,255,255,0.02);
  }

  tr.selected {
    background: rgba(231, 196, 107, 0.05);
  }

  tr.selected td {
    color: var(--accent-primary);
  }

  td {
    padding: 14px 16px;
    color: var(--text-secondary);
  }

  .col-status { width: 60px; text-align: center; }
  
  .status-indicator {
    display: inline-flex;
    color: var(--text-tertiary);
  }

  .status-indicator.ready {
    color: var(--accent-success);
  }

  .status-indicator.indexed {
    color: #3296ff;
  }

  .title-cell {
    display: flex;
    align-items: center;
    gap: 12px;
    font-weight: 500;
  }

  .badge {
    padding: 2px 8px;
    border-radius: 4px;
    font-size: 11px;
    font-weight: 700;
  }

  .agency-badge {
    background: rgba(255,255,255,0.05);
    color: var(--text-tertiary);
  }

  .source-link {
    color: var(--text-tertiary);
    transition: color 0.2s;
  }

  .source-link:hover {
    color: var(--accent-primary);
  }

  .empty-state {
    padding: 60px;
    text-align: center;
    color: var(--text-tertiary);
    font-style: italic;
  }
</style>