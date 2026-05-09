<script lang="ts">
  import type { RecordSummary } from "$lib/types";

  let { records, selectedRecordId = null, onSelect } = $props<{
    records: RecordSummary[];
    selectedRecordId?: string | null;
    onSelect: (record: RecordSummary) => void;
  }>();
</script>

<div class="cards-view">
  {#each records as record}
     <button class="intel-card-item glass-panel" class:active={selectedRecordId === record.id} onclick={() => onSelect(record)}>
       <span class="card-agency">{record.agency || "Unknown Agency"}</span>
       <h3>{record.title}</h3>
       <p>{record.summary || "No summary available."}</p>
       <div class="card-meta">
         <span>{record.source_type}</span>
         <span class="status-pill" class:completed={record.analysis_status === 'completed'}>{record.analysis_status || 'pending'}</span>
       </div>
     </button>
  {/each}
</div>

<style>
  .cards-view {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(320px, 1fr));
    gap: 20px;
    padding: 24px;
  }
  .intel-card-item {
    text-align: left;
    padding: 24px;
    transition: transform 0.2s, border-color 0.2s;
    background: var(--bg-surface);
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-md);
    cursor: pointer;
    color: var(--text-primary);
  }
  .intel-card-item:hover {
    transform: translateY(-2px);
    border-color: rgba(231, 196, 107, 0.4);
  }
  .intel-card-item.active {
    border-color: var(--accent-primary);
    box-shadow: 0 0 0 1px var(--accent-primary);
  }
  .card-agency {
    font-size: 10px;
    text-transform: uppercase;
    color: var(--accent-primary);
    display: block;
    margin-bottom: 12px;
    letter-spacing: 0.1em;
  }
  .intel-card-item h3 {
    font-size: 16px;
    margin-bottom: 12px;
    line-height: 1.4;
  }
  .intel-card-item p {
    font-size: 13px;
    color: var(--text-secondary);
    margin-bottom: 24px;
    display: -webkit-box;
    -webkit-line-clamp: 3;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .card-meta {
    display: flex;
    justify-content: space-between;
    align-items: center;
    font-size: 11px;
    color: var(--text-secondary);
    text-transform: uppercase;
  }
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
