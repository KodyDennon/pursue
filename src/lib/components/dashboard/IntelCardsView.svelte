<script lang="ts">
  import { convertFileSrc } from "@tauri-apps/api/core";
  import type { RecordSummary } from "$lib/types";
  import { FileText, MapPin, Calendar, Database } from "lucide-svelte";

  let { records, selectedRecordId = null, onSelect } = $props<{
    records: RecordSummary[];
    selectedRecordId?: string | null;
    onSelect: (record: RecordSummary) => void;
  }>();
</script>

<div class="cards-view">
  {#each records as record}
      <button class="intel-card-item glass-panel" class:active={selectedRecordId === record.id} onclick={() => onSelect(record)}>
        {#if record.thumbnail_path}
          <div class="card-thumbnail">
            <img src={convertFileSrc(record.thumbnail_path)} alt="Evidence thumbnail" />
            <div class="thumbnail-overlay"></div>
          </div>
        {:else}
          <div class="card-thumbnail-placeholder">
            <FileText size={32} class="placeholder-icon" />
          </div>
        {/if}
        
        <div class="card-content">
          <span class="card-agency">{record.agency || "Unknown Agency"}</span>
          <h3>{record.title}</h3>
          <p>{record.summary || "No summary available."}</p>
          
          <div class="card-details">
            {#if record.incident_location}
              <div class="detail-item">
                <MapPin size={12} />
                <span>{record.incident_location}</span>
              </div>
            {/if}
            {#if record.incident_date}
              <div class="detail-item">
                <Calendar size={12} />
                <span>{record.incident_date}</span>
              </div>
            {/if}
          </div>

          <div class="card-meta">
            <div class="source-tag">
              <Database size={10} />
              <span>{record.source_type}</span>
            </div>
            <span class="status-pill" class:completed={record.analysis_status === 'completed'}>{record.analysis_status || 'pending'}</span>
          </div>
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
    display: flex;
    flex-direction: column;
    transition: transform 0.2s, border-color 0.2s, background 0.2s;
    background: var(--bg-surface);
    border: 1px solid var(--border-subtle);
    border-radius: 12px;
    cursor: pointer;
    color: var(--text-primary);
    overflow: hidden;
    height: 100%;
  }
  .intel-card-item:hover {
    transform: translateY(-4px);
    border-color: var(--accent-primary);
    background: var(--bg-surface-elevated);
  }
  .intel-card-item.active {
    border-color: var(--accent-primary);
    box-shadow: 0 0 0 2px rgba(231, 196, 107, 0.2);
    background: var(--bg-surface-elevated);
  }

  .card-thumbnail {
    width: 100%;
    aspect-ratio: 16/9;
    position: relative;
    overflow: hidden;
    background: #000;
  }
  .card-thumbnail img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    transition: transform 0.4s ease;
  }
  .intel-card-item:hover .card-thumbnail img {
    transform: scale(1.05);
  }
  .thumbnail-overlay {
    position: absolute;
    top: 0; left: 0; right: 0; bottom: 0;
    background: linear-gradient(to bottom, transparent 60%, rgba(0,0,0,0.8));
  }

  .card-thumbnail-placeholder {
    width: 100%;
    aspect-ratio: 16/9;
    display: flex;
    align-items: center;
    justify-content: center;
    background: #111;
    color: var(--text-tertiary);
  }
  .placeholder-icon { opacity: 0.2; }

  .card-content {
    padding: 20px;
    display: flex;
    flex-direction: column;
    flex: 1;
  }

  .card-agency {
    font-size: 9px;
    text-transform: uppercase;
    color: var(--accent-primary);
    display: block;
    margin-bottom: 8px;
    letter-spacing: 0.15em;
    font-weight: 700;
  }
  .intel-card-item h3 {
    font-size: 15px;
    margin-bottom: 12px;
    line-height: 1.4;
    font-weight: 600;
  }
  .intel-card-item p {
    font-size: 13px;
    color: var(--text-secondary);
    margin-bottom: 16px;
    display: -webkit-box;
    -webkit-line-clamp: 3;
    -webkit-box-orient: vertical;
    overflow: hidden;
    line-height: 1.5;
    flex: 1;
  }

  .card-details {
    display: flex;
    flex-direction: column;
    gap: 8px;
    margin-bottom: 20px;
  }
  .detail-item {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 11px;
    color: var(--text-tertiary);
  }

  .card-meta {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding-top: 16px;
    border-top: 1px solid var(--border-subtle);
  }
  .source-tag {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 10px;
    color: var(--text-tertiary);
    text-transform: uppercase;
  }

  .status-pill {
    background: rgba(255,255,255,0.05);
    padding: 2px 8px;
    border-radius: 4px;
    font-size: 9px;
    text-transform: uppercase;
    font-weight: 700;
    letter-spacing: 0.05em;
  }
  .status-pill.completed {
    background: rgba(77, 243, 169, 0.1);
    color: var(--accent-success);
    border: 1px solid rgba(77, 243, 169, 0.2);
  }
</style>
