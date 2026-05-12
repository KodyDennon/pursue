<script lang="ts">
  import GridView from "./GridView.svelte";
  import IntelCardsView from "./IntelCardsView.svelte";
  import ListView from "./ListView.svelte";
  import IntelligenceDossier from "../IntelligenceDossier.svelte";
  import type { CaseSummary, RecordSummary } from "$lib/types";

  let { 
    records, 
    libraryPath, 
    viewMode, 
    cases, 
    selectedCaseId,
    selectedRecord = $bindable(null),
    onChanged,
    onAnalyze,
    onViewMedia
  } = $props<{
    records: RecordSummary[];
    libraryPath: string | null;
    viewMode: "grid" | "cards" | "list";
    cases: CaseSummary[];
    selectedCaseId: string | null;
    selectedRecord: RecordSummary | null;
    onChanged: () => void | Promise<void>;
    onAnalyze: () => void;
    onViewMedia: (record: RecordSummary) => void;
  }>();
</script>

<div class="dashboard-container">
  {#if selectedRecord}
    <IntelligenceDossier 
      record={selectedRecord} 
      libraryPath={libraryPath}
      cases={cases}
      selectedCaseId={selectedCaseId}
      onBack={() => (selectedRecord = null)}
      onChanged={onChanged}
      onAnalyze={onAnalyze}
    />
  {:else}
    {#if viewMode === 'grid'}
      <GridView 
        records={records} 
        libraryPath={libraryPath}
        selectedRecordId={selectedRecord?.id}
        onSelect={(r) => (selectedRecord = r)}
        onView={onViewMedia}
      />
    {:else if viewMode === 'cards'}
      <IntelCardsView 
        records={records} 
        libraryPath={libraryPath}
        selectedRecordId={selectedRecord?.id}
        onSelect={(r) => (selectedRecord = r)}
        onView={onViewMedia}
      />
    {:else if viewMode === 'list'}
      <ListView 
        records={records} 
        libraryPath={libraryPath}
        selectedRecordId={selectedRecord?.id}
        onSelect={(r) => (selectedRecord = r)}
        onView={onViewMedia}
      />
    {/if}
  {/if}
</div>

<style>
  .dashboard-container {
    height: 100%;
    width: 100%;
    display: flex;
    flex-direction: column;
  }
</style>
