<script lang="ts">
  import type { Record } from "$lib/types";

  let { record, onBack } = $props<{ record: Record, onBack: () => void }>();
</script>

<div class="h-full flex flex-col bg-zinc-950">
  <header class="p-4 border-b border-zinc-800 flex items-center gap-4 bg-zinc-900/50">
    <button onclick={onBack} class="text-zinc-500 hover:text-white transition-colors">← Back to Index</button>
    <div class="h-4 w-px bg-zinc-800"></div>
    <h2 class="font-mono text-blue-400 uppercase tracking-tighter truncate">{record.title}</h2>
  </header>

  <div class="flex-1 flex overflow-hidden">
    <!-- Left: Document/Media View -->
    <div class="flex-[2] border-r border-zinc-800 bg-zinc-900/20 overflow-y-auto p-8 flex flex-col items-center gap-8">
      <div class="w-full max-w-3xl aspect-[1/1.414] bg-white shadow-2xl rounded-sm flex items-center justify-center text-zinc-400 font-serif p-12 relative overflow-hidden">
        <!-- Document Placeholder -->
        <div class="absolute inset-0 opacity-5 pointer-events-none select-none overflow-hidden text-[8px] leading-tight">
          {record.summary?.repeat(100)}
        </div>
        <div class="text-center">
          <p class="text-zinc-800 text-2xl mb-4 font-bold uppercase tracking-widest border-b-2 border-zinc-800 pb-2">Declassified Document</p>
          <p class="text-zinc-500 mb-8 italic">Cleared for release under PURSUE Directive 2026-01</p>
          <div class="space-y-4 text-left font-mono text-sm text-zinc-700">
            <p><span class="bg-zinc-200 px-1">AGENCY:</span> {record.agency}</p>
            <p><span class="bg-zinc-200 px-1">RELEASE:</span> {record.release_date}</p>
            <p><span class="bg-zinc-200 px-1">INCIDENT:</span> {record.incident_date}</p>
            <p><span class="bg-zinc-200 px-1">LOCATION:</span> {record.incident_location}</p>
          </div>
          <div class="mt-12 p-4 border-4 border-red-900/20 text-red-900/40 rotate-12 font-black text-4xl border-dashed">
            UNRESOLVED
          </div>
        </div>
      </div>

      {#if record.document_url}
        <a 
          href={record.document_url} 
          target="_blank" 
          class="px-6 py-3 bg-blue-600 hover:bg-blue-500 text-white rounded-lg font-bold shadow-lg transition-all hover:scale-105"
        >
          View Original Source Material
        </a>
      {/if}
    </div>

    <!-- Right: Intelligence Sidebar -->
    <div class="flex-1 bg-zinc-900/40 overflow-y-auto p-6 flex flex-col gap-8">
      <section>
        <h3 class="text-xs font-bold text-zinc-500 uppercase tracking-widest mb-4">Relational Entities</h3>
        <div class="flex flex-wrap gap-2">
          {#if record.agency}
            <span class="px-2 py-1 bg-blue-900/30 text-blue-400 border border-blue-900/50 rounded text-xs font-mono">{record.agency}</span>
          {/if}
          {#if record.incident_location}
             <span class="px-2 py-1 bg-emerald-900/30 text-emerald-400 border border-emerald-900/50 rounded text-xs font-mono">{record.incident_location}</span>
          {/if}
          <span class="px-2 py-1 bg-zinc-800 text-zinc-500 border border-zinc-700 rounded text-xs font-mono italic">+ Identify new entities</span>
        </div>
      </section>

      <section>
        <h3 class="text-xs font-bold text-zinc-500 uppercase tracking-widest mb-4">Deep Analysis Summary</h3>
        <div class="p-4 bg-zinc-950 border border-zinc-800 rounded-lg">
          <p class="text-sm leading-relaxed text-zinc-300">
            {record.summary || 'No detailed analysis has been performed on this record yet. Click "Analyze" to begin background OCR and semantic indexing.'}
          </p>
        </div>
      </section>

      <section>
        <h3 class="text-xs font-bold text-zinc-500 uppercase tracking-widest mb-4">Internal Notes</h3>
        <textarea 
          placeholder="Add your investigation notes here..." 
          class="w-full h-32 bg-zinc-950 border border-zinc-800 rounded-lg p-3 text-sm focus:outline-none focus:border-blue-500 transition-colors"
        ></textarea>
      </section>

      <div class="mt-auto space-y-2">
        <button class="w-full py-2 bg-zinc-800 hover:bg-zinc-700 rounded-md text-sm font-medium transition-colors">Add to Case</button>
        <button class="w-full py-2 bg-zinc-800 hover:bg-zinc-700 rounded-md text-sm font-medium transition-colors">Generate Dossier Link</button>
      </div>
    </div>
  </div>
</div>
