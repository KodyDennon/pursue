<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import type { Record } from "$lib/types";
  import Map from "$lib/components/Map.svelte";
  import ArchiveViewer from "$lib/components/ArchiveViewer.svelte";

  let records = $state<Record[]>([]);
  let loading = $state(true);
  let syncing = $state(false);
  let viewMode = $state<'list' | 'map'>('list');
  let selectedRecord = $state<Record | null>(null);

  async function loadRecords() {
    loading = true;
    try {
      records = await invoke<Record[]>("get_records");
    } catch (e) {
      console.error("Failed to load records", e);
    } finally {
      loading = false;
    }
  }

  async function syncRecords() {
    syncing = true;
    try {
      const count = await invoke<number>("sync_records");
      console.log(`Synced ${count} new records`);
      await loadRecords();
    } catch (e) {
      console.error("Failed to sync records", e);
    } finally {
      syncing = false;
    }
  }

  onMount(() => {
    loadRecords();
  });
</script>

<div class="min-h-screen bg-zinc-950 text-zinc-100 flex flex-col font-sans">
  <!-- Top Bar -->
  <header class="h-16 border-b border-zinc-800 flex items-center justify-between px-6 bg-zinc-900/50 backdrop-blur-md sticky top-0 z-10">
    <div class="flex items-center gap-4">
      <div class="w-8 h-8 bg-blue-600 rounded-sm flex items-center justify-center font-bold text-lg">P</div>
      <h1 class="text-xl font-bold tracking-tight uppercase">Pursue <span class="text-zinc-500 font-light">Data Analyzer</span></h1>
    </div>
    <div class="flex items-center gap-4">
      <button 
        onclick={syncRecords} 
        disabled={syncing}
        class="bg-blue-600 hover:bg-blue-500 disabled:bg-zinc-800 text-white px-4 py-2 rounded-md text-sm font-medium transition-colors flex items-center gap-2"
      >
        {#if syncing}
          <span class="animate-spin text-lg">⟳</span> Syncing...
        {:else}
          Sync Official Data
        {/if}
      </button>
      <div class="w-10 h-10 rounded-full bg-zinc-800 border border-zinc-700"></div>
    </div>
  </header>

  <main class="flex-1 flex overflow-hidden">
    <!-- Sidebar -->
    <aside class="w-64 border-r border-zinc-800 bg-zinc-900/30 p-4 flex flex-col gap-6 overflow-y-auto">
      <nav class="flex flex-col gap-2">
        <button class="w-full text-left px-3 py-2 rounded-md bg-zinc-800 text-blue-400 font-medium">All Intelligence</button>
        <button class="w-full text-left px-3 py-2 rounded-md hover:bg-zinc-800/50 text-zinc-400">Official Records</button>
        <button class="w-full text-left px-3 py-2 rounded-md hover:bg-zinc-800/50 text-zinc-400">Personal Evidence</button>
      </nav>

      <div>
        <h3 class="text-xs font-semibold text-zinc-500 uppercase tracking-widest px-3 mb-2">Investigations</h3>
        <button class="w-full text-left px-3 py-2 rounded-md hover:bg-zinc-800/50 text-zinc-400 flex items-center gap-2">
          <span class="text-lg">+</span> Create New Case
        </button>
      </div>

      <div class="mt-auto p-4 bg-blue-900/10 border border-blue-900/30 rounded-lg">
        <p class="text-xs text-blue-300/70 mb-1">DATA INTEGRITY</p>
        <p class="text-sm font-mono text-blue-400">SECURE_ACTIVE</p>
      </div>
    </aside>

    <!-- Content -->
    <div class="flex-1 flex flex-col overflow-hidden">
      <!-- Search & Filters -->
      <div class="p-4 border-b border-zinc-800 bg-zinc-900/20 flex items-center gap-4">
        <div class="relative flex-1">
          <input 
            type="text" 
            placeholder="Search metadata, OCR text, and entities..." 
            class="w-full bg-zinc-900 border border-zinc-700 rounded-lg px-4 py-2 focus:outline-none focus:border-blue-500 transition-colors"
          />
        </div>
        <select class="bg-zinc-900 border border-zinc-700 rounded-lg px-3 py-2 text-sm">
          <option>All Agencies</option>
          <option>FBI</option>
          <option>NASA</option>
          <option>DOW</option>
        </select>
        <button 
          onclick={() => viewMode = viewMode === 'list' ? 'map' : 'list'}
          class="px-4 py-2 bg-zinc-800 hover:bg-zinc-700 rounded-lg text-sm transition-colors border border-zinc-700"
        >
          {viewMode === 'list' ? 'Geospatial View' : 'Record List View'}
        </button>
      </div>

      <!-- List or Map View -->
      <div class="flex-1 overflow-hidden">
        {#if selectedRecord}
          <ArchiveViewer 
            record={selectedRecord} 
            onBack={() => selectedRecord = null} 
          />
        {:else if loading}
          <div class="flex items-center justify-center h-full text-zinc-500 italic">
            Initializing data streams...
          </div>
        {:else if records.length === 0}
          <div class="flex flex-col items-center justify-center h-full text-zinc-500 gap-4">
            <p>No records found in local database.</p>
            <button onclick={syncRecords} class="text-blue-500 hover:underline">Start initial sync</button>
          </div>
        {:else if viewMode === 'map'}
          <Map {records} />
        {:else}
          <div class="h-full overflow-y-auto">
            <table class="w-full text-left border-collapse">
              <thead class="sticky top-0 bg-zinc-900 z-10 border-b border-zinc-800 text-xs font-semibold text-zinc-500 uppercase tracking-widest">
                <tr>
                  <th class="px-6 py-3">Asset Title</th>
                  <th class="px-6 py-3">Agency</th>
                  <th class="px-6 py-3">Release</th>
                  <th class="px-6 py-3">Incident</th>
                  <th class="px-6 py-3 text-right">Action</th>
                </tr>
              </thead>
              <tbody class="divide-y divide-zinc-800/50">
                {#each records as record}
                  <tr 
                    onclick={() => selectedRecord = record}
                    class="hover:bg-blue-600/5 cursor-pointer group transition-colors"
                  >
                    <td class="px-6 py-4">
                      <div class="flex flex-col">
                        <span class="font-medium text-zinc-200 group-hover:text-blue-400 transition-colors">{record.title}</span>
                        <span class="text-xs text-zinc-500 truncate max-w-md">{record.summary || 'No summary available'}</span>
                      </div>
                    </td>
                    <td class="px-6 py-4">
                      <span class="px-2 py-0.5 rounded-sm bg-zinc-800 text-xs font-mono">{record.agency || 'UNKNOWN'}</span>
                    </td>
                    <td class="px-6 py-4 text-sm text-zinc-400">{record.release_date || 'N/A'}</td>
                    <td class="px-6 py-4 text-sm text-zinc-400">{record.incident_date || 'N/A'}</td>
                    <td class="px-6 py-4 text-right">
                      <button 
                        onclick={(e) => { e.stopPropagation(); selectedRecord = record; }}
                        class="text-xs font-semibold text-zinc-500 hover:text-blue-400 uppercase tracking-widest"
                      >
                        Analyze
                      </button>
                    </td>
                  </tr>
                {/each}
              </tbody>
            </table>
          </div>
        {/if}
      </div>
    </div>
  </main>

  <!-- Footer Stats -->
  <footer class="h-8 bg-zinc-900 border-t border-zinc-800 px-6 flex items-center justify-between text-[10px] font-mono text-zinc-500">
    <div class="flex gap-6">
      <span>TOTAL_RECORDS: {records.length}</span>
      <span>INDEX_STATUS: {loading ? 'UPDATING' : 'IDLE'}</span>
    </div>
    <div>
      <span>SYSTEM_TIME: {new Date().toISOString()}</span>
    </div>
  </footer>
</div>

<style>
  :global(body) {
    margin: 0;
    overflow: hidden;
  }
</style>
