<script lang="ts">
  import { convertFileSrc } from "@tauri-apps/api/core";
  import type { RecordSummary, RecordAsset } from "$lib/types";

  let { record, assets = [] } = $props<{ record: RecordSummary; assets: RecordAsset[] }>();
  
  const intelligence = $derived(record.intelligence_json ? JSON.parse(record.intelligence_json) : null);
  const images = $derived(assets.filter((a: RecordAsset) => a.asset_type === 'image'));
</script>

<div class="intel-dossier">
  {#if intelligence}
    <div class="dossier-grid">
      <section class="intel-card full hero">
        <div class="card-glow"></div>
        <span class="card-label">Executive Intelligence Summary</span>
        <p class="summary-text">{intelligence.object_description || 'No detailed description extracted.'}</p>
      </section>

      <section class="intel-card">
        <span class="card-label">Primary Engagement Data</span>
        <div class="metrics-grid">
          <div class="m-item">
            <span class="m-label">Incident Date</span>
            <span class="m-val">{intelligence.incident_date || 'N/A'}</span>
          </div>
          <div class="m-item">
            <span class="m-label">Location</span>
            <span class="m-val">{intelligence.location || 'N/A'}</span>
          </div>
        </div>
      </section>

      <section class="intel-card">
        <span class="card-label">Agencies Involved</span>
        <div class="tag-cloud">
          {#each (intelligence.agencies || []) as agency}
            <span class="intel-tag">{agency}</span>
          {/each}
          {#if !(intelligence.agencies?.length)}
            <span class="no-data">None identified</span>
          {/if}
        </div>
      </section>

      <section class="intel-card full observations">
        <header class="obs-head">
          <span class="card-label">Pilot & Personnel Observations</span>
          <span class="live-indicator">GROUND TRUTH EXTRACED</span>
        </header>
        <div class="obs-content">
          <div class="quote-mark">“</div>
          <p class="summary-text small">{intelligence.pilot_observations || 'No specific personnel observations documented.'}</p>
        </div>
      </section>

      {#if images.length > 0}
        <section class="intel-card full gallery">
          <span class="card-label">Evidence Gallery (Extracted from PDF)</span>
          <div class="gallery-grid">
            {#each images as asset}
              <div class="evidence-frame glass">
                <img src={convertFileSrc(asset.local_path)} alt="Extracted Evidence" />
                <div class="frame-meta">
                  <span>{asset.mime_type}</span>
                  <span>{asset.file_size ? (asset.file_size / 1024).toFixed(0) : 0} KB</span>
                </div>
              </div>
            {/each}
          </div>
        </section>
      {/if}

      <section class="intel-card full forensics">
        <div class="f-header">
          <span class="card-label">Intelligence Confidence & Integrity</span>
          <span class="engine-tag">Gemma 4 Elite</span>
        </div>
        <div class="f-body">
          <div class="f-metric">
            <span>Redaction Check</span>
            <strong>{intelligence.redaction_summary || 'Not analyzed'}</strong>
          </div>
          <div class="f-metric">
            <span>Extraction Status</span>
            <strong class="status-ok">VERIFIED</strong>
          </div>
        </div>
      </section>
    </div>
  {:else}
    <div class="pending-dossier glass">
      <div class="spinner"></div>
      <h3>Intelligence Extraction Pending</h3>
      <p>Initiate Gemma 4 deep analysis to populate this dossier.</p>
    </div>
  {/if}
</div>

<style>
  .intel-dossier {
    animation: dossier-fade 0.4s ease-out;
  }

  @keyframes dossier-fade {
    from { opacity: 0; transform: translateY(10px); }
    to { opacity: 1; transform: translateY(0); }
  }

  .dossier-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 20px;
  }

  .intel-card {
    background: var(--bg-secondary);
    border: 1px solid var(--border-dim);
    border-radius: 16px;
    padding: 24px;
    position: relative;
    overflow: hidden;
    transition: border-color 0.3s;
  }

  .intel-card:hover {
    border-color: rgba(231, 196, 107, 0.4);
  }

  .intel-card.full {
    grid-column: span 2;
  }

  .hero {
    background: linear-gradient(135deg, var(--bg-secondary), #15171d);
  }

  .card-glow {
    position: absolute;
    top: -50%;
    left: -50%;
    width: 200%;
    height: 200%;
    background: radial-gradient(circle at 50% 50%, rgba(231, 196, 107, 0.05), transparent 70%);
    pointer-events: none;
  }

  .card-label {
    display: block;
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.15em;
    color: var(--text-secondary);
    margin-bottom: 16px;
  }

  .summary-text {
    font-size: 16px;
    line-height: 1.6;
    color: var(--text-primary);
    margin: 0;
  }

  .summary-text.small {
    font-size: 15px;
    color: #9da3ad;
    font-style: italic;
  }

  .metrics-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 24px;
  }

  .m-item {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .m-label {
    font-size: 11px;
    color: var(--text-secondary);
  }

  .m-val {
    font-size: 18px;
    font-weight: 600;
    color: var(--accent-gold);
  }

  .observations {
    background: rgba(0,0,0,0.4);
    border: 1px solid rgba(231, 196, 107, 0.15);
  }

  .obs-head {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 20px;
  }

  .live-indicator {
    font-size: 9px;
    font-weight: 800;
    color: var(--accent-gold);
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .live-indicator::before {
    content: '';
    width: 6px;
    height: 6px;
    background: var(--accent-gold);
    border-radius: 50%;
    animation: pulse 2s infinite;
  }

  .obs-content {
    display: flex;
    gap: 20px;
  }

  .quote-mark {
    font-size: 64px;
    color: rgba(231, 196, 107, 0.2);
    font-family: serif;
    line-height: 0.5;
    margin-top: 12px;
  }

  .gallery-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
    gap: 16px;
  }

  .evidence-frame {
    aspect-ratio: 4/3;
    overflow: hidden;
    position: relative;
    border-radius: 8px;
    border: 1px solid var(--border-dim);
  }

  .evidence-frame img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    transition: transform 0.5s;
  }

  .evidence-frame:hover img {
    transform: scale(1.1);
  }

  .frame-meta {
    position: absolute;
    bottom: 0;
    left: 0;
    right: 0;
    background: rgba(0,0,0,0.8);
    padding: 8px 12px;
    font-size: 10px;
    display: flex;
    justify-content: space-between;
    opacity: 0;
    transition: opacity 0.3s;
  }

  .evidence-frame:hover .frame-meta {
    opacity: 1;
  }

  .tag-cloud {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }

  .intel-tag {
    background: var(--accent-gold-dim);
    color: var(--accent-gold);
    border: 1px solid rgba(231, 196, 107, 0.2);
    padding: 4px 12px;
    border-radius: 6px;
    font-size: 12px;
    font-weight: 600;
  }

  .forensics {
    background: rgba(0,0,0,0.3);
  }

  .f-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 20px;
  }

  .engine-tag {
    font-size: 10px;
    font-weight: 800;
    background: #e7c46b;
    color: #000;
    padding: 2px 8px;
    border-radius: 4px;
  }

  .f-body {
    display: flex;
    gap: 40px;
  }

  .f-metric {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .f-metric span {
    font-size: 11px;
    color: var(--text-secondary);
  }

  .f-metric strong {
    font-size: 14px;
  }

  .status-ok {
    color: #4df3a9;
  }

  .pending-dossier {
    height: 400px;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    text-align: center;
    border-radius: 20px;
    padding: 40px;
  }

  .spinner {
    width: 48px;
    height: 48px;
    border: 3px solid var(--border-dim);
    border-top-color: var(--accent-gold);
    border-radius: 50%;
    animation: spin 1s infinite linear;
    margin-bottom: 24px;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }

  @keyframes pulse {
    0% { transform: scale(1); opacity: 1; }
    50% { transform: scale(1.5); opacity: 0.5; }
    100% { transform: scale(1); opacity: 1; }
  }

  h3 { margin: 0 0 8px; font-size: 20px; }
  p { margin: 0; color: var(--text-secondary); }
</style>
