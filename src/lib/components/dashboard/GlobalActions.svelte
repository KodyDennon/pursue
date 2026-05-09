<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { open } from "@tauri-apps/plugin-dialog";
  import { addToast, updateToast } from "$lib/toastStore";
  import type { RecordSummary } from "$lib/types";
  import { Loader2 } from "lucide-svelte";

  let { query = $bindable(""), onLoad, onSelect, onSync, busy = $bindable(null) } = $props<{
    query: string;
    onLoad: () => Promise<void>;
    onSelect: (record: RecordSummary) => void;
    onSync: () => Promise<void>;
    busy?: string | null;
  }>();

  let webUrl = $state("");
  let showWebIngest = $state(false);



  async function importFile() {
    busy = "import";
    try {
      const selected = await open({
        multiple: false,
        filters: [{ name: "Evidence", extensions: ["pdf", "txt", "md", "csv", "json", "png", "jpg"] }]
      });
      if (typeof selected !== "string") {
        busy = null;
        return;
      }
      
      const toastId = addToast({ type: "loading", message: "Ingesting file...", duration: 0 });
      try {
        const record = await invoke<RecordSummary>("import_manual_file", { request: { path: selected, title: null, notes: null } });
        await onLoad();
        onSelect(record);
        updateToast(toastId, { type: "success", message: "File imported successfully", duration: 3000 });
      } catch (e) {
        updateToast(toastId, { type: "error", message: `Import failed: ${e}`, duration: 5000 });
      }
    } catch (e) {
      addToast({ type: "error", message: `Dialog error: ${e}` });
    } finally {
      busy = null;
    }
  }

  async function ingestWeb() {
    if (!webUrl.trim()) {
      showWebIngest = false;
      return;
    }
    busy = "web";
    const toastId = addToast({ type: "loading", message: "Scraping web page...", duration: 0 });
    try {
      const record = await invoke<RecordSummary>("ingest_web_page", { url: webUrl.trim() });
      await onLoad();
      onSelect(record);
      webUrl = "";
      showWebIngest = false;
      updateToast(toastId, { type: "success", message: "Web page ingested", duration: 3000 });
    } catch (e) {
      updateToast(toastId, { type: "error", message: `Scraping failed: ${e}`, duration: 5000 });
    } finally {
      busy = null;
    }
  }
</script>

<div class="global-actions">
  <div class="search-wrap">
    <input 
      type="text" 
      bind:value={query} 
      placeholder="Cmd+K to Search" 
      onkeydown={(e) => { if (e.key === 'Enter') { e.preventDefault(); onLoad(); } }}
    />
  </div>
  
  {#if showWebIngest}
    <div class="search-wrap">
      <input type="text" bind:value={webUrl} placeholder="https://..." onkeydown={(e) => { if (e.key === 'Enter') ingestWeb(); }} />
      <button class="action-btn" onclick={ingestWeb} disabled={busy === 'web'}>
        {#if busy === 'web'}
          <Loader2 size={14} class="spin" />
        {:else}
          Fetch
        {/if}
      </button>
    </div>
  {:else}
    <button class="action-btn" onclick={() => showWebIngest = true}>Web Ingest</button>
  {/if}
  
  <button class="action-btn" onclick={onSync} disabled={busy === 'sync'}>
    {#if busy === 'sync'}
      <Loader2 size={14} class="spin" /> Syncing...
    {:else}
      Sync WAR.gov
    {/if}
  </button>
  
  <button class="action-btn primary" onclick={importFile} disabled={busy === 'import'}>
    {#if busy === 'import'}
      <Loader2 size={14} class="spin" /> Importing...
    {:else}
      Ingest File
    {/if}
  </button>
</div>

<style>
  .global-actions {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .search-wrap {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .search-wrap input {
    background: rgba(0,0,0,0.4);
    border: 1px solid var(--border-subtle);
    padding: 8px 16px;
    border-radius: var(--radius-sm);
    color: var(--text-primary);
    width: 280px;
    font-size: 13px;
  }
  
  .search-wrap input:focus {
    border-color: var(--accent-primary);
    outline: none;
  }

  .action-btn {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 16px;
    background: var(--bg-surface);
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-sm);
    font-size: 13px;
    transition: var(--transition-fast);
    cursor: pointer;
  }

  .action-btn:hover:not(:disabled) { border-color: var(--accent-primary); }
  .action-btn:disabled { opacity: 0.6; cursor: not-allowed; }
  .action-btn.primary { background: var(--accent-primary); color: #000; font-weight: 600; border: none; }

  :global(.spin) {
    animation: spin 1s linear infinite;
  }
  @keyframes spin {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
  }
</style>
