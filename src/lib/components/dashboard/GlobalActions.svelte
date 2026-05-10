<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { open } from "@tauri-apps/plugin-dialog";
  import { addToast, updateToast } from "$lib/toastStore";
  import type { RecordSummary } from "$lib/types";
  import { Loader2, Grid, LayoutList, LayoutGrid, X, Globe, RefreshCcw, FilePlus, Zap, Brain } from "lucide-svelte";
  import { activeView } from "$lib/store";

  let { query = $bindable(""), onLoad, onSelect, onSync, onAnalyze, busy = $bindable(null), viewMode = $bindable("grid") } = $props<{
    query: string;
    onLoad: () => Promise<void>;
    onSelect: (record: RecordSummary) => void;
    onSync: () => Promise<void>;
    onAnalyze: () => void;
    busy?: string | null;
    viewMode: "grid" | "cards" | "list";
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
  {#if $activeView === 'dashboard'}
    <div class="view-toggles">
      <button class="toggle-btn" class:active={viewMode === 'grid'} onclick={() => viewMode = 'grid'} title="Grid View">
        <LayoutGrid size={16} />
      </button>
      <button class="toggle-btn" class:active={viewMode === 'cards'} onclick={() => viewMode = 'cards'} title="Card View">
        <Grid size={16} />
      </button>
      <button class="toggle-btn" class:active={viewMode === 'list'} onclick={() => viewMode = 'list'} title="List View">
        <LayoutList size={16} />
      </button>
    </div>
  {/if}

  <div class="search-wrap">
    <input 
      type="text" 
      bind:value={query} 
      placeholder="Semantic Search..." 
      onkeydown={(e) => { if (e.key === 'Enter') { e.preventDefault(); onLoad(); } }}
    />
    {#if query}
      <button class="clear-search" onclick={() => { query = ""; onLoad(); }} title="Clear Search">
        <X size={14} />
      </button>
    {/if}
  </div>
  
  <div class="action-group">
    {#if showWebIngest}
      <div class="web-ingest-expand">
        <input type="text" bind:value={webUrl} placeholder="https://..." onkeydown={(e) => { if (e.key === 'Enter') ingestWeb(); }} />
        <button class="icon-action-btn success" onclick={ingestWeb} disabled={busy === 'web'}>
          {#if busy === 'web'}
            <Loader2 size={16} class="spin" />
          {:else}
            <Zap size={16} />
          {/if}
        </button>
        <button class="icon-action-btn" onclick={() => showWebIngest = false}><X size={16} /></button>
      </div>
    {:else}
      <button class="action-btn" onclick={() => showWebIngest = true} title="Web Ingest">
        <Globe size={16} /> <span>Web</span>
      </button>
    {/if}
    
    <button class="action-btn" onclick={onSync} disabled={busy === 'sync'} title="Sync Official Sources">
      {#if busy === 'sync'}
        <Loader2 size={16} class="spin" />
      {:else}
        <RefreshCcw size={16} />
      {/if}
      <span>Sync</span>
    </button>
    
    <button class="action-btn" onclick={onAnalyze} title="Neural Deep Scan">
      <Brain size={16} />
      <span>Analyze</span>
    </button>
    
    <button class="action-btn primary" onclick={importFile} disabled={busy === 'import'} title="Ingest Local File">
      {#if busy === 'import'}
        <Loader2 size={16} class="spin" />
      {:else}
        <FilePlus size={16} />
      {/if}
      <span>Ingest</span>
    </button>
  </div>
</div>

<style>
  .global-actions {
    display: flex;
    align-items: center;
    gap: 16px;
  }

  .view-toggles {
    display: flex;
    background: rgba(0,0,0,0.2);
    padding: 2px;
    border-radius: var(--radius-md);
    border: 1px solid var(--border-subtle);
  }

  .toggle-btn {
    width: 32px;
    height: 32px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: var(--radius-sm);
    color: var(--text-tertiary);
    transition: all 0.2s;
    cursor: pointer;
    background: none;
    border: none;
  }

  .toggle-btn:hover { color: var(--text-primary); }
  .toggle-btn.active {
    background: var(--bg-surface-elevated);
    color: var(--accent-primary);
    box-shadow: 0 1px 3px rgba(0,0,0,0.3);
  }

  .search-wrap {
    display: flex;
    align-items: center;
    gap: 8px;
    position: relative;
  }

  .search-wrap input {
    background: rgba(0,0,0,0.4);
    border: 1px solid var(--border-subtle);
    padding: 8px 36px 8px 16px;
    border-radius: var(--radius-sm);
    color: var(--text-primary);
    width: 240px;
    font-size: 13px;
    transition: border-color 0.2s, width 0.3s;
  }
  
  .search-wrap input:focus {
    border-color: var(--accent-primary);
    outline: none;
    width: 320px;
  }

  .clear-search {
    position: absolute;
    right: 10px;
    color: var(--text-tertiary);
    background: none;
    border: none;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .clear-search:hover { color: var(--text-primary); }

  .action-group {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .web-ingest-expand {
    display: flex;
    align-items: center;
    gap: 4px;
    background: var(--bg-surface-elevated);
    padding: 2px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--accent-primary);
  }

  .web-ingest-expand input {
    background: transparent;
    border: none;
    padding: 4px 8px;
    color: var(--text-primary);
    font-size: 12px;
    width: 180px;
    outline: none;
  }

  .icon-action-btn {
    width: 28px;
    height: 28px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: var(--radius-xs);
    background: none;
    border: none;
    color: var(--text-tertiary);
    cursor: pointer;
  }
  .icon-action-btn:hover { background: rgba(255,255,255,0.05); color: var(--text-primary); }
  .icon-action-btn.success { color: var(--accent-success); }

  .action-btn {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 12px;
    background: var(--bg-surface);
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-sm);
    font-size: 12px;
    font-weight: 600;
    transition: var(--transition-fast);
    cursor: pointer;
    color: var(--text-secondary);
  }

  .action-btn:hover:not(:disabled) { 
    border-color: var(--accent-primary); 
    color: var(--text-primary);
  }
  .action-btn:disabled { opacity: 0.6; cursor: not-allowed; }
  .action-btn.primary { background: var(--accent-primary); color: #000; border: none; }

  :global(.spin) {
    animation: spin 1s linear infinite;
  }
  @keyframes spin {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
  }
</style>