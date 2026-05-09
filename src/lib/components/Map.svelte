<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import type { RecordSummary } from "$lib/types";
  import L from "leaflet";
  import "leaflet/dist/leaflet.css";

  let { records = [], onSelect = null } = $props<{ records: RecordSummary[]; onSelect?: (record: RecordSummary) => void }>();
  let mapElement: HTMLDivElement;
  let map: L.Map | null = null;
  let markerLayer: L.LayerGroup | null = null;

  const knownLocations: Array<[RegExp, [number, number]]> = [
    [/kazakhstan/i, [48.0196, 66.9237]],
    [/papua new guinea/i, [-6.315, 143.9555]],
    [/\bgeorgia\b/i, [32.1656, -82.9001]],
    [/mexico/i, [23.6345, -102.5528]],
    [/middle east/i, [29.2985, 42.551]],
    [/united states|usa|u\.s\./i, [37.0902, -95.7129]],
    [/nevada/i, [38.8026, -116.4194]],
    [/new mexico/i, [34.5199, -105.8701]],
    [/arizona/i, [34.0489, -111.0937]],
    [/california/i, [36.7783, -119.4179]],
    [/florida/i, [27.6648, -81.5158]],
    [/atlantic/i, [31.0, -45.0]],
    [/pacific/i, [8.7832, -124.5085]]
  ];

  onMount(() => {
    map = L.map(mapElement, {
      center: [24, -34],
      zoom: 2,
      zoomControl: false,
      attributionControl: false
    });

    L.tileLayer("https://{s}.basemaps.cartocdn.com/dark_all/{z}/{x}/{y}{r}.png", {
      maxZoom: 19
    }).addTo(map);

    markerLayer = L.layerGroup().addTo(map);
    updateMarkers(records);
  });

  function coordinatesFor(location: string | null): [number, number] | null {
    if (!location) return null;
    for (const [pattern, coords] of knownLocations) {
      if (pattern.test(location)) return coords;
    }
    return null;
  }

  function updateMarkers(nextRecords: RecordSummary[]) {
    if (!map || !markerLayer) return;
    markerLayer.clearLayers();

    for (const record of nextRecords) {
      const coords = coordinatesFor(record.incident_location);
      if (!coords) continue;

      const markerIcon = L.divIcon({
        className: "tactical-pip",
        html: "<div class='pulse-dot'></div>",
        iconSize: [20, 20],
        iconAnchor: [10, 10]
      });

      L.marker(coords, { icon: markerIcon })
        .addTo(markerLayer)
        .bindPopup(`
          <div class="tactical-popup">
            <header>
              <strong>${record.title}</strong>
              <span class="status ${record.analysis_status}">${record.analysis_status || 'Pending'}</span>
            </header>
            <p>${record.agency}</p>
            <div class="p-footer">
              <small>${record.incident_date || 'N/A'}</small>
              <button class="mini-btn" onclick="window.dispatchEvent(new CustomEvent('select-record', { detail: '${record.id}' }))">Open Dossier</button>
            </div>
          </div>
        `, {
          className: 'tactical-popup-container'
        });
    }
  }

  onMount(() => {
    window.addEventListener('select-record', ((e: CustomEvent<string>) => {
      const record = records.find((r: RecordSummary) => r.id === e.detail);
      if (record && onSelect) onSelect(record);
    }) as EventListener);
  });

  $effect(() => {
    updateMarkers(records);
  });

  onDestroy(() => {
    if (map) map.remove();
  });
</script>

<div bind:this={mapElement} class="map-surface"></div>

<style>
  .map-surface {
    width: 100%;
    height: 100%;
    background: #0a0b0d;
  }

  :global(.tactical-pip) {
    background: transparent;
    border: none;
  }

  :global(.pulse-dot) {
    width: 12px;
    height: 12px;
    background: #e7c46b;
    border-radius: 50%;
    box-shadow: 0 0 15px #e7c46b;
    border: 2px solid #0a0b0d;
    position: relative;
  }

  :global(.pulse-dot::after) {
    content: '';
    position: absolute;
    width: 100%;
    height: 100%;
    top: 0;
    left: 0;
    background: #e7c46b;
    border-radius: 50%;
    animation: pip-pulse 2s infinite;
    z-index: -1;
  }

  @keyframes pip-pulse {
    0% { transform: scale(1); opacity: 0.8; }
    100% { transform: scale(3); opacity: 0; }
  }

  :global(.leaflet-popup-content-wrapper) {
    background: rgba(16, 17, 20, 0.9) !important;
    backdrop-filter: blur(10px);
    border: 1px solid rgba(231, 196, 107, 0.3);
    color: white !important;
    border-radius: 12px !important;
    padding: 0 !important;
    box-shadow: 0 12px 24px rgba(0,0,0,0.5) !important;
  }

  :global(.leaflet-popup-tip) {
    background: rgba(16, 17, 20, 0.9) !important;
    border: 1px solid rgba(231, 196, 107, 0.3);
  }

  :global(.tactical-popup) {
    padding: 16px;
    min-width: 240px;
    font-family: 'Outfit', sans-serif;
  }

  :global(.tactical-popup header) {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: 12px;
    margin-bottom: 8px;
  }

  :global(.tactical-popup strong) {
    font-size: 14px;
    font-weight: 700;
  }

  :global(.tactical-popup .status) {
    font-size: 9px;
    text-transform: uppercase;
    padding: 2px 6px;
    border-radius: 4px;
    background: rgba(255,255,255,0.1);
  }

  :global(.tactical-popup p) {
    margin: 0;
    font-size: 12px;
    color: #9da3ad;
  }

  :global(.tactical-popup .p-footer) {
    margin-top: 12px;
    padding-top: 8px;
    border-top: 1px solid rgba(255,255,255,0.1);
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  :global(.mini-btn) {
    background: var(--accent-gold);
    color: #000;
    border: none;
    border-radius: 4px;
    padding: 4px 8px;
    font-size: 10px;
    font-weight: 800;
    cursor: pointer;
  }
</style>
