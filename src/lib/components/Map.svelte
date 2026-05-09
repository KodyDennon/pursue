<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import type { RecordSummary } from "$lib/types";
  import L from "leaflet";
  import "leaflet/dist/leaflet.css";

  let { records = [] } = $props<{ records: RecordSummary[] }>();
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

    L.control.zoom({ position: "bottomright" }).addTo(map);
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
        className: "custom-div-icon",
        html: "<div class='marker-pin'></div>",
        iconSize: [14, 14],
        iconAnchor: [7, 7]
      });

      L.marker(coords, { icon: markerIcon })
        .addTo(markerLayer)
        .bindPopup(`
          <div class="pursue-popup">
            <strong>${escapeHtml(record.title)}</strong>
            <span>${escapeHtml(record.agency || "UNKNOWN")} · ${escapeHtml(record.incident_date || "N/A")}</span>
          </div>
        `);
    }
  }

  function escapeHtml(value: string): string {
    return value
      .replaceAll("&", "&amp;")
      .replaceAll("<", "&lt;")
      .replaceAll(">", "&gt;")
      .replaceAll('"', "&quot;");
  }

  $effect(() => {
    updateMarkers(records);
  });

  onDestroy(() => {
    if (map) map.remove();
  });
</script>

<div bind:this={mapElement} class="map"></div>

<style>
  .map {
    width: 100%;
    height: 100%;
    min-height: 0;
    background: #101114;
  }

  :global(.custom-div-icon) {
    background: transparent;
    border: none;
  }

  :global(.marker-pin) {
    width: 14px;
    height: 14px;
    border-radius: 50%;
    background: #e7c46b;
    border: 2px solid #101114;
    box-shadow: 0 0 0 2px rgba(231, 196, 107, 0.28), 0 0 18px rgba(231, 196, 107, 0.72);
  }

  :global(.leaflet-popup-content-wrapper) {
    background: transparent !important;
    box-shadow: none !important;
    padding: 0 !important;
  }

  :global(.leaflet-popup-tip) {
    display: none !important;
  }

  :global(.pursue-popup) {
    display: grid;
    gap: 4px;
    min-width: 210px;
    color: #f4f1e8;
    background: #17191e;
    border: 1px solid #3a3d45;
    border-radius: 6px;
    padding: 10px;
  }

  :global(.pursue-popup span) {
    color: #9da3ad;
    font-size: 12px;
  }
</style>
