<script lang="ts">
  import type { RecordSummary } from "$lib/types";

  let { records, selectedRecordId = null, onSelect } = $props<{
    records: RecordSummary[];
    selectedRecordId?: string | null;
    onSelect: (record: RecordSummary) => void;
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

<div class="grid-view">
  <table>
    <thead>
      <tr>
        <th>Title</th>
        <th>Agency</th>
        <th>Status</th>
        <th>Source</th>
        <th>Size</th>
      </tr>
    </thead>
    <tbody>
      {#each records as record}
        <tr class:selected={selectedRecordId === record.id} onclick={() => onSelect(record)}>
          <td>
            <strong>{record.title}</strong>
            <small>{record.incident_location || "Unknown location"}</small>
          </td>
          <td>{record.agency || "Unknown"}</td>
          <td>
            <span class="status-pill" class:completed={record.analysis_status === 'completed'}>
              {record.analysis_status || 'pending'}
            </span>
          </td>
          <td>{record.source_type}</td>
          <td>{record.local_path ? formatBytes(record.artifact_size) : "Remote"}</td>
        </tr>
      {/each}
    </tbody>
  </table>
</div>

<style>
  .grid-view table {
    width: 100%;
    border-collapse: collapse;
  }
  .grid-view th {
    text-align: left;
    padding: 12px 24px;
    font-size: 11px;
    text-transform: uppercase;
    color: var(--text-secondary);
    border-bottom: 1px solid var(--border-subtle);
    background: var(--bg-surface);
    position: sticky;
    top: 0;
    z-index: 1;
  }
  .grid-view td {
    padding: 16px 24px;
    border-bottom: 1px solid rgba(255,255,255,0.03);
    font-size: 14px;
  }
  .grid-view tr {
    transition: background 0.2s;
    cursor: pointer;
  }
  .grid-view tr:hover { background: rgba(255,255,255,0.02); }
  .grid-view tr.selected { background: rgba(231, 196, 107, 0.05); }

  .grid-view td strong { display: block; margin-bottom: 4px; color: var(--text-primary); }
  .grid-view td small { color: var(--text-secondary); font-size: 12px; }

  .status-pill {
    background: rgba(255,255,255,0.1);
    padding: 2px 8px;
    border-radius: 12px;
    font-size: 10px;
    text-transform: uppercase;
  }
  .status-pill.completed {
    background: rgba(77, 243, 169, 0.1);
    color: var(--accent-success);
    border: 1px solid rgba(77, 243, 169, 0.2);
  }
</style>
