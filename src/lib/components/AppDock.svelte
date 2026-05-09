<script lang="ts">
  import { Map, Database, BrainCircuit, Settings, Search } from "lucide-svelte";
  import Logo from "$lib/components/Logo.svelte";
  import { activeView, globalSearchOpen } from "$lib/store";
</script>

<nav class="dock">
  <div class="dock-top">
    <div class="brand">
      <Logo size={32} class="brand-icon" />
    </div>
    
    <div class="nav-items">
      <button class="nav-item" class:active={$activeView === 'grid'} onclick={() => $activeView = 'grid'} title="Evidence Grid">
        <Database size={22} />
      </button>
      <button class="nav-item" class:active={$activeView === 'map'} onclick={() => $activeView = 'map'} title="Tactical Map">
        <Map size={22} />
      </button>
      <button class="nav-item" class:active={$activeView === 'link-analysis'} onclick={() => $activeView = 'link-analysis'} title="Link Analysis">
        <BrainCircuit size={22} />
      </button>
      <button class="nav-item" onclick={() => $globalSearchOpen = true} title="Global Search">
        <Search size={22} />
      </button>
    </div>
  </div>

  <div class="dock-bottom">
    <button class="nav-item" title="Settings">
      <Settings size={22} />
    </button>
  </div>
</nav>

<style>
  .dock {
    width: 68px;
    height: 100%;
    background-color: var(--bg-surface);
    border-right: 1px solid var(--border-subtle);
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    align-items: center;
    padding: 24px 0;
    z-index: 100;
  }

  .dock-top, .dock-bottom {
    display: flex;
    flex-direction: column;
    align-items: center;
    width: 100%;
    gap: 24px;
  }

  .brand {
    color: var(--text-primary);
    margin-bottom: 12px;
  }
  
  :global(.brand-icon) {
    color: var(--accent-primary);
  }

  .nav-items {
    display: flex;
    flex-direction: column;
    gap: 16px;
    width: 100%;
    align-items: center;
  }

  .nav-item {
    width: 48px;
    height: 48px;
    border-radius: var(--radius-md);
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-secondary);
    transition: var(--transition-fast);
    position: relative;
  }

  .nav-item:hover {
    color: var(--text-primary);
    background-color: var(--bg-surface-elevated);
  }

  .nav-item.active {
    color: var(--accent-primary);
    background-color: rgba(231, 196, 107, 0.1);
  }
  
  .nav-item.active::before {
    content: '';
    position: absolute;
    left: -10px;
    top: 50%;
    transform: translateY(-50%);
    height: 24px;
    width: 3px;
    background-color: var(--accent-primary);
    border-radius: 0 4px 4px 0;
  }
</style>
