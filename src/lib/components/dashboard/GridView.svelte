<script lang="ts">
  import type { RecordSummary } from "$lib/types";
  import { FileText, MapPin, Calendar, CheckCircle2, Clock } from "lucide-svelte";

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

<div class="cards-view custom-scrollbar">
  <div class="cards-grid">
    {#each records as record}
      <button 
        class="evidence-card" 
        class:selected={selectedRecordId === record.id} 
        onclick={() => onSelect(record)}
      >
        <div class="card-glow"></div>
        <header class="card-header">
          <div class="type-icon">
            <FileText size={16} />
          </div>
          <span class="agency-tag">{record.agency || "Unknown"}</span>
          <div class="status-indicator" class:completed={record.analysis_status === 'completed'}>
            {#if record.analysis_status === 'completed'}
              <CheckCircle2 size={12} />
            {:else}
              <Clock size={12} />
            {/if}
            <span>{record.analysis_status || 'pending'}</span>
          </div>
        </header>

        <div class="card-body">
          <h3>{record.title}</h3>
          <div class="meta-row">
            <div class="meta-item">
              <MapPin size={12} />
              <span>{record.incident_location || "Unknown location"}</span>
            </div>
            <div class="meta-item">
              <Calendar size={12} />
              <span>{record.release_date || "Undated"}</span>
            </div>
          </div>
        </div>

        <footer class="card-footer">
          <span class="file-info">{record.file_type || 'PDF'} • {record.local_path ? formatBytes(record.artifact_size) : "Cloud Source"}</span>
          <div class="intel-tag" class:active={record.analysis_status === 'completed'}>
            INTELLIGENCE READY
          </div>
        </footer>
      </button>
    {/each}
  </div>
</div>

<style>
  .cards-view {
    height: 100%;
    overflow-y: auto;
    padding: 24px;
    box-sizing: border-box;
  }

  .cards-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(320px, 1fr));
    gap: 20px;
  }

  .evidence-card {
    background: rgba(255,255,255,0.02);
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-lg);
    padding: 20px;
    display: flex;
    flex-direction: column;
    gap: 16px;
    text-align: left;
    position: relative;
    overflow: hidden;
    transition: var(--transition-normal);
  }

  .evidence-card:hover {
    background: rgba(255,255,255,0.04);
    border-color: var(--accent-primary);
    transform: translateY(-2px);
  }

  .evidence-card.selected {
    background: rgba(231, 196, 107, 0.05);
    border-color: var(--accent-primary);
    box-shadow: 0 0 20px rgba(231, 196, 107, 0.1);
  }

  .card-glow {
    position: absolute;
    top: 0;
    left: 0;
    width: 100%;
    height: 100%;
    background: radial-gradient(circle at 50% 0%, rgba(231, 196, 107, 0.05), transparent 70%);
    opacity: 0;
    transition: opacity 0.3s ease;
  }

  .evidence-card:hover .card-glow { opacity: 1; }

  .card-header {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .type-icon {
    width: 28px;
    height: 28px;
    border-radius: 8px;
    background: rgba(255,255,255,0.05);
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-secondary);
  }

  .agency-tag {
    font-size: 11px;
    font-weight: 700;
    color: var(--text-tertiary);
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .status-indicator {
    margin-left: auto;
    display: flex;
    align-items: center;
    gap: 4px;
    font-size: 10px;
    text-transform: uppercase;
    font-weight: 700;
    padding: 2px 8px;
    border-radius: 4px;
    background: rgba(255,255,255,0.05);
    color: var(--text-tertiary);
  }

  .status-indicator.completed {
    background: rgba(77, 243, 169, 0.1);
    color: var(--accent-success);
  }

  .card-body h3 {
    font-size: 16px;
    font-weight: 600;
    margin: 0;
    color: var(--text-primary);
    line-height: 1.4;
    display: -webkit-box;
    line-clamp: 2;
    -webkit-line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }

  .meta-row {
    margin-top: 12px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .meta-item {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12px;
    color: var(--text-secondary);
  }

  .card-footer {
    margin-top: auto;
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding-top: 16px;
    border-top: 1px solid rgba(255,255,255,0.03);
  }

  .file-info {
    font-size: 11px;
    color: var(--text-tertiary);
  }

  .intel-tag {
    font-size: 9px;
    font-weight: 800;
    letter-spacing: 0.1em;
    padding: 2px 6px;
    border-radius: 2px;
    background: rgba(255,255,255,0.05);
    color: rgba(255,255,255,0.1);
    transition: var(--transition-normal);
  }

  .intel-tag.active {
    background: var(--accent-primary);
    color: #000;
    box-shadow: 0 0 10px rgba(231, 196, 107, 0.3);
  }
</style>

