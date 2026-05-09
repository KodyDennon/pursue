<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import type { Record } from "$lib/types";
  import L from "leaflet";
  import "leaflet/dist/leaflet.css";

  let { records = [] } = $props<{ records: Record[] }>();
  let mapElement: HTMLDivElement;
  let map: L.Map;

  onMount(() => {
    map = L.map(mapElement, {
      center: [20, 0],
      zoom: 2,
      zoomControl: false,
      attributionControl: false
    });

    // Dark theme for OSM using CartoDB
    L.tileLayer('https://{s}.basemaps.cartocdn.com/dark_all/{z}/{x}/{y}{r}.png', {
      maxZoom: 19
    }).addTo(map);

    L.control.zoom({ position: 'bottomright' }).addTo(map);

    updateMarkers();
  });

  function updateMarkers() {
    if (!map) return;
    
    // Clear existing markers if needed (simplified for now)
    
    records.forEach(record => {
      if (record.incident_location && record.incident_location !== "N/A") {
        // In a real app, we'd geocode these. For now, we'll look for coords in the text or mock.
        // For the demo, let's randomly place them if they have a location but no coords.
        // The government CSV doesn't have lat/long yet, so we'll need a geocoder or pre-mapped data.
        
        // Mocking coordinates for demo if location is "Kazakhstan", "Moon", etc.
        let coords: [number, number] | null = null;
        if (record.incident_location.includes("Kazakhstan")) coords = [48.0196, 66.9237];
        if (record.incident_location.includes("Papua New Guinea")) coords = [-6.314993, 143.95555];
        if (record.incident_location.includes("Georgia")) coords = [42.3154, 43.3569];
        if (record.incident_location.includes("Mexico")) coords = [23.6345, -102.5528];
        if (record.incident_location.includes("Middle East")) coords = [29.2985, 42.5510];
        if (record.incident_location.includes("United States")) coords = [37.0902, -95.7129];
        
        if (coords) {
          const markerIcon = L.divIcon({
            className: 'custom-div-icon',
            html: `<div class='marker-pin bg-blue-500 shadow-[0_0_10px_rgba(59,130,246,0.8)]'></div>`,
            iconSize: [12, 12],
            iconAnchor: [6, 6]
          });

          L.marker(coords, { icon: markerIcon })
            .addTo(map)
            .bindPopup(`
              <div class="bg-zinc-900 text-white p-2 border border-zinc-700 rounded-md">
                <h3 class="font-bold text-blue-400">${record.title}</h3>
                <p class="text-xs text-zinc-400">${record.agency} | ${record.incident_date}</p>
              </div>
            `);
        }
      }
    });
  }

  $effect(() => {
    if (records.length > 0) {
      updateMarkers();
    }
  });

  onDestroy(() => {
    if (map) map.remove();
  });
</script>

<div bind:this={mapElement} class="w-full h-full bg-zinc-900"></div>

<style>
  :global(.custom-div-icon) {
    background: transparent;
    border: none;
  }
  :global(.marker-pin) {
    width: 12px;
    height: 12px;
    border-radius: 50%;
    border: 2px solid white;
  }
  :global(.leaflet-popup-content-wrapper) {
    background: transparent !important;
    box-shadow: none !important;
    padding: 0 !important;
  }
  :global(.leaflet-popup-tip) {
    display: none !important;
  }
</style>
