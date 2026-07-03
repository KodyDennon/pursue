# PURSUE Data Analyzer — 3-Phase Overhaul (war.gov sync, performance, UI/UX)

## Context

PURSUE Data Analyzer (Tauri v2 + Svelte 5 + Rust, `/home/user/projects/pursue`) syncs UAP/UFO
disclosure records from `war.gov/UFO` (the "PURSUE" federal disclosure program). The Department
of War has published 3 releases (May 8, May 22, June 12 2026 — 294 records), and Release 3 added
new contributing agencies (FBI/CIA/NASA) with an independent `FBI-UAP-PR` title-numbering series,
alongside the existing `DOW-UAP-PR`/`DOW-UAP-D` series. The repo's last commit is 2026-05-26 —
before Release 3 — so the sync path has never been exercised against it, and existing test
fixtures only cover Release 2's shape.

Research (4 parallel audits: architecture, bug hunt, performance, UI/UX) found the app's sync
architecture is actually fairly resilient already (it discovers the live CSV URL dynamically
rather than hardcoding it — see `src/lib/warGovSource.ts`), but turned up real correctness bugs
in the ingestion pipeline, real performance problems (some already causing "download stalling"
symptoms seen in past commits), and a UI layer that shows clear signs of having been built
incrementally by different agents with no shared design system — Tailwind v4 is installed but
unused, 72% of components hardcode colors instead of using the existing CSS tokens, and there are
4 independent hand-rolled modal implementations with no shared base.

Decision: execute as three sequential phases — correctness first (highest risk if wrong), then
performance, then a full UI/UX unification retrofit across all 58 components, adopting Tailwind
v4 properly via an `@theme` token block. Each phase lands as its own reviewable, working state
before the next begins.

---

## PHASE 1 — Sync/Ingestion Correctness

Goal: make sync atomic and observable, fix silent download-classification failures, add Release 3
regression coverage. Do the transaction fix first — everything else in this phase touches code
that runs inside that same sync call and should be written against the transactional signature
from the start.

### 1.1 — Make sync atomic (highest severity, do first)

`src-tauri/src/sources/war_gov.rs`, `sync_official_source_from_bytes_inner` (line 43):

- Currently the `source_snapshots` row is inserted with a hardcoded `status='completed'` and the
  full `record_count` (lines 62-79) **before** the per-record loop (86-142) that actually
  populates `source_snapshot_records`/`source_diffs`/`records`. No transaction exists anywhere in
  this function or in `upsert_record`/`insert_diff` (confirmed via grep — zero `begin()`/`BEGIN`
  hits). A mid-loop failure leaves a snapshot flagged `completed` with a mismatched child-row
  count, and `previous_snapshot_records()` (line 619) picks it as the next sync's diff baseline —
  permanently corrupting future diff history.
- Fix: open `let mut tx = pool.begin().await?` right after computing the hash/writing the
  snapshot file (keep file I/O outside the DB transaction). Insert `source_snapshots` with
  `status='pending'` inside `tx`. Change `upsert_record` (line 311) and `insert_diff` (line 649)
  signatures from `pool: &SqlitePool` to `tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>`, updating
  every `.execute(pool)` call inside them to `.execute(&mut **tx)`. Thread `&mut tx` through both
  loops (86-142, 144-176) and the raw queries inline in them. Switch `prior_title` (line 682) to
  read via the same `tx` for snapshot isolation. After the removed-records loop succeeds, `UPDATE
  source_snapshots SET status='completed' WHERE id=?` inside `tx`, then `tx.commit().await?`.
- **Do not** fold `repair_official_record_identities` (called as a pre-step at line 49) into this
  transaction — it repairs pre-existing data corruption and is already idempotent/safe to
  interrupt. It's worth its own transaction wrapping (one per duplicate-group iteration in
  `merge_duplicate_record`, lines 479-607, not one giant transaction) but as a separate sub-task,
  not gated on 1.1.
- Both `upsert_record`/`insert_diff` are private fns used only within `war_gov.rs` — signature
  change is self-contained to this file.
- Add a DB-backed test asserting the atomicity fix: inject a mid-loop failure via a fixture and
  assert `source_snapshots` is never left `completed` with a mismatched
  `source_snapshot_records` count. Check for an existing `sqlx::test`/in-memory-DB test helper
  elsewhere in the crate before inventing a new one.

### 1.2 — Fail loudly on CSV column drift

`src-tauri/src/sources/war_gov.rs` (`parse_csv_records`, line 192), `src/lib/warGovSource.ts`
(`validateWarGovCsv`, line 31):

- `get_field` (war_gov.rs:239-244) silently returns `None` for any missing/renamed column. If a
  future release renames `"PDF | Image Link"` or `"DVIDS Video ID"`, every record gets
  `document_url=None`/`dvids_video_id=None`, `source_asset_class()` (735-763) classifies
  everything `"metadata_only"`, `downloadable_source_url()` (`commands/records.rs:1240-1254`)
  returns `None` for every row, and `download_missing_records` silently skips all of them — sync
  reports "success" with the correct record count while every download silently no-ops.
- Fix: after building `header_map` (216-221), hard-fail (`Err(anyhow!(...))`, same pattern as the
  existing HTML-sniff check at 193-200) if any of `"PDF | Image Link"`, `"DVIDS Video ID"`,
  `"Type"`, `"Modal Image"` is absent. Don't hard-fail on cosmetic/pairing-only columns.
  Mirror the same required-column list in `validateWarGovCsv` (warGovSource.ts:31-38), extending
  the existing `requiredHeaders` array.
- Add a test dropping `"PDF | Image Link"` from a fixture and asserting `Err`.

### 1.3 — Fix `file_type` (already broken today, not release-specific)

`src-tauri/src/sources/war_gov.rs` (`upsert_record`, ~line 376), `src-tauri/src/library/mod.rs`
(`ingest_part_file`, line 168):

- `upsert_record` binds the raw CSV `Type` code (`"VID"`/`"AUD"`/`"IMG"`/`"PDF"`) into
  `records.file_type` at sync time, before any file is downloaded. `MediaViewer.svelte:71-76`
  checks `file_type` against real extensions (`['mp4','webm',...]`, `['jpg','png',...]`) — `"vid"`
  never matches, so every downloaded video/audio/image hits "Unsupported Preview Type" today.
  Only `"PDF"` coincidentally matches.
- Fix at the source of truth: `ingest_part_file` already computes the real extension (168-178).
  After `attach_artifact` succeeds, add `UPDATE records SET file_type=? WHERE id=?` using that
  extension. Apply the same fix in `ingest_manual_file` and `ingest_from_bytes` for parity.
  `MediaViewer.svelte` needs zero changes — its extension checks already work once `file_type`
  holds a real value.
- Optional/nice-to-have (don't block Phase 1 on it): a one-time backfill migration for already-
  downloaded records whose `file_type` is still wrong from before this fix.

### 1.4 — DVIDS resolver: stop hardcoding the `video` namespace

`src-tauri/src/commands/records.rs` (`resolve_dvids_metadata` line 1026,
`build_dvids_resolver_script` line 1108), `src/lib/downloads/downloadWorker.ts`,
`downloadWorkerCore.ts`:

- `build_dvids_resolver_script` always requests `id=video:{id}` regardless of actual type, but
  `source_asset_class` already distinguishes `"dvids_video"` vs `"dvids_audio"`. A genuine
  DVIDS-hosted audio record fails to resolve today.
- Add an `asset_type` param threaded from the record's `source_asset_class` through
  `resolve_dvids_metadata` → `build_dvids_resolver_script`, interpolating `audio:`/`video:` per
  DVIDS's namespace convention. Extend `buildResolveDvidsMetadataArgs`
  (downloadWorkerCore.ts:56-58) and `resolveSource` (downloadWorker.ts:256-284) to pass it
  through — verify `BulkDownloadItem`/the download-queue row actually carries
  `source_asset_class`; extend the SELECT in `download_missing_records`/`queue_record_download`
  if not.
- Extend the existing `build_war_gov_download_script` test block (records.rs:1279-1293) with an
  audio-type assertion.

### 1.5 — Non-war.gov/DVIDS host fallback (stretch item)

No new agency host is confirmed WAF-protected yet — don't build a speculative bypass. Instead
make the failure mode diagnosable: confirm `classifyDownloadError`
(downloadWorkerCore.ts:24-54) already classifies a 403 from an unexpected host as `'blocked'`,
and that `DownloadAgent.svelte` visually distinguishes `'blocked'` from `'network'` errors. Add
that distinction if missing.

### 1.6 — Lower-priority correctness batch (do after 1.1-1.4)

- `downloadable_source_url` (records.rs:1240-1254) always prefers `document_url` over
  `dvids_video_id` when both are set, silently dropping the video. Minimal fix: log a warning
  when this happens so it's observable; defer real dual-asset download modeling.
- Add `modal_image` as a final fallback in `downloadable_source_url` only when `document_url` is
  empty (it's a thumbnail, never preferred over a full-res `document_url`).
- Reorder `war_gov.rs:56-59` so `parse_csv_records` runs before the snapshot file is written to
  disk, so a parse failure doesn't orphan a snapshot file.
- Split `sync()` in `+page.svelte:130-186` into two try/catch blocks: one around the CSV
  fetch+`sync_official_source_with_csv` call (the real "sync failed" case), one around the
  post-sync cleanup/download-trigger steps (so a `cleanup_duplicates` failure doesn't get
  misreported as "sync failed"). Reuse the existing `addToast`/`updateToast` pattern.
- `canonical_document_identity` dropping the URL query string, and the `stable_key` metadata-
  fallback collision risk: leave as documented known limitations unless Release 3 fixtures (1.8)
  demonstrate an actual collision.

### 1.7 — Surface the added/changed/removed diff UI (feature gap)

`SyncReport.diffs` is already computed and persisted to `source_diffs`, but
`invoke('sync_official_source_with_csv', ...)` at `+page.svelte:151` discards the return value,
and nothing queries `source_diffs` back out.

- Minimal fix: capture the return value (`const report = await invoke<SyncReport>(...)`) and
  extend the existing success toast to show real added/changed/removed counts.
- Add a `get_latest_sync_diffs(limit)` Tauri command reusing the `ORDER BY fetched_at DESC LIMIT
  1` pattern from `previous_snapshot_records` (war_gov.rs:619-624); register it in `lib.rs`'s
  `invoke_handler!`. Render it as local `$state` in `+page.svelte` plus a small new
  `SyncDiffSummary.svelte` (reusing existing `.glass-panel` styling) rather than adding a 9th
  store for a single-consumer feature.

### 1.8 — Release 3 test fixtures (interleave early, verify 1.1-1.4 against them as they land)

`src-tauri/src/sources/war_gov.rs` `#[cfg(test)]` module (881-945):

- Add a `RELEASE_03_STYLE_CSV` fixture modeled on `CURRENT_STYLE_CSV` (885-888), with FBI/CIA/NASA
  agency values, `FBI-UAP-PR###` titles, and `.../medialink/ufo/061226/release_03/documents/...`
  URLs (confirm exact agency-name strings against a live sync during Phase 1 manual verification
  before finalizing).
- Test that `parse_csv_records` handles the new agencies, `release_labels_for` correctly derives
  `"Release 03"` from date ordering (already generic — this test confirms it, doesn't add logic),
  and `stable_key` doesn't collide between `FBI-UAP-PR001` and `DOW-UAP-PR001`.
- Test that `source_asset_class` classification is agency-agnostic (doesn't accidentally scope to
  `agency`).

### Phase 1 verification

**Automated**: `cd src-tauri && cargo check` (will surface every remaining `pool`-typed call site
you missed in 1.1), `cargo test sources::war_gov`, `cargo test commands::records`, `cargo test`
(full suite), `bun run check` from repo root.

**Manual (`bun tauri dev`)**:
1. Trigger a real sync against live war.gov data. Confirm via `sqlite3 <app_data_dir>/pursue.db
   "SELECT DISTINCT agency FROM records WHERE source_type='official'"` that FBI/CIA/NASA appear.
2. Confirm `FBI-UAP-PR` and `DOW-UAP-PR` records have distinct `stable_key`s.
3. Force a mid-sync failure (corrupt/truncate the CSV) and confirm no `source_snapshots` row ends
   up `completed` with a mismatched `source_snapshot_records` count.
4. Confirm `download_missing_records` actually queues Release 3 documents (skipped count stays
   low, matching only genuinely metadata-only records).
5. Open a downloaded video/audio/image record in `MediaViewer` and confirm it renders (validates
   1.3 end-to-end).
6. Confirm the sync success toast shows real added/changed/removed counts (1.7).

---

## PHASE 2 — Performance

Goal: cut boot IPC round-trips, fix the broken search debounce, unblock "load more," stop the
chunk-per-DB-write download bottleneck. No visual changes (that's Phase 3). Do the isolated
one-file fixes first (2.1, 2.2), then boot dedup (2.3-2.4) together since they touch the same
sequence, then the higher-risk download rewrite (2.5) once the app is otherwise stable to test
against.

### 2.1 — Fix unreachable "load more" (functional bug, do first)

`src/routes/+page.svelte`: `loadMoreRecords()` (120-128) is correct, but its only button
(418-425) is nested inside `activeView==='map' && selectedRecord` — unreachable from the primary
Grid/List/Cards dashboard view, hard-capping users at 250 records with no way to see the rest of
the archive. Read `Dashboard.svelte`'s prop surface first, then either pass
`recordsTotal`/`records.length`/`onLoadMore` into it, or render the existing `.load-more-row`
markup (539-559, reuse as-is) as a sibling of `<Dashboard>` gated on `activeView==='dashboard'`.

### 2.2 — Fix broken search debounce

`src/lib/components/GlobalSearch.svelte:69-74`: `oninput` schedules a `setTimeout` without
clearing the previous one, so 3 keystrokes fire 3 overlapping full ONNX embedding searches. Fix:
hoist a `debounceTimer` variable, `clearTimeout` before rescheduling. Also guard against
out-of-order results: snapshot the query at call time and only apply results if the query hasn't
changed since. Flag (don't necessarily fix) `search.rs:200`'s synchronous `session.run()` inside
an async fn under a single global `Mutex<Session>` as a follow-up if overlap issues persist after
the debounce fix.

### 2.3 — Deduplicate boot sequence

`src/routes/+page.svelte` (`onMount` 195-232, `loadInitialData` 55-118),
`src/lib/stores/intelligenceStore.svelte.ts` (`init` 46, `loadStatus` 123): boot currently does
~15 sequential IPC calls including exact duplicates — `intelligenceStore.init()` calls
`loadStatus()` once, then `+page.svelte:206-209` re-invokes `check_model_status`/
`get_hardware_diagnostics` redundantly, then `loadInitialData()` calls `loadStatus()` a third
time. Delete the redundant standalone invokes at 206-209 in favor of reading
`intelligenceStore.diagnostics`/`.models` (already `$state`, already populated). Pick one
authoritative call site for the status refresh across all 4 call sites of `loadInitialData`
(onMount, sync(), FirstLaunch onComplete, the activeView `$effect`) rather than three. Where the
3 independent loads (`list_records_page`, `list_cases`, status refresh) don't depend on each
other, reintroduce `Promise.all` — but check `git show a924e54` first for why it was made
sequential, to avoid reintroducing whatever race it was fixing. Once genuinely deduplicated, the
15s force-clear `setTimeout` band-aid (61-72) should rarely fire — keep it as defense-in-depth,
don't remove it, but its near-total silence becomes the manual verification signal.

### 2.4 — Reduce `get_database_status` polling frequency

`intelligenceStore.svelte.ts:48` polls a 17-subquery status command every 5s forever regardless
of active view. Prefer pausing the interval when the Intelligence Center view isn't active
(add `pause()`/`resume()` methods to the store, call from `+page.svelte`'s existing
`activeView` `$effect`) over restructuring the query itself — the subqueries are already
reasonably index-covered; frequency is the actual issue.

### 2.5 — Chunked download rewrite (highest-effort item, do after 2.1-2.4 are stable)

`src-tauri/src/downloads.rs` (`DownloadPartWriter::append`, line 38-57),
`src-tauri/src/commands/records.rs` (`download_war_gov_item_with_webview` 546-763,
`build_war_gov_download_script` 1142-1238). Sequence carefully — this is the item most likely to
regress download stability if rushed:

- **2.5a**: `DownloadPartWriter::append` reopens the file (`OpenOptions::new().append(true)
  .open()`) on every single chunk call. Give `DownloadPartWriter` a persistent open file handle
  held across the writer's lifetime instead. Leave `finalize()`'s hashing pass unchanged unless
  trivial to also fix. Existing tests (`downloads.rs:82-145`) must still pass unmodified.
- **2.5b**: raise the hardcoded 64KB chunk size in `build_war_gov_download_script`'s `emitChunk`
  (line ~1168) incrementally to 256KB (matching the already-proven-stable `MAX_CHUNK_BYTES` used
  by the non-webview path in `downloadWorker.ts:13`) rather than jumping further — this value was
  likely tuned down for webview-bridge stability reasons, so move cautiously.
- **2.5c**: reuse the exact time/byte-threshold pattern already in `downloadWorkerCore.ts`'s
  `getProgressUpdate` (500ms / 1MB) to throttle the `Chunk` handler's DB `UPDATE` in
  `records.rs:674-700` — still write every chunk to disk immediately, just not every chunk's
  progress to the DB.
- **2.5d**: falls out for free once 2.5a lands, since `append_download_chunk` (446-480) shares the
  same `DownloadPartWriter`.
- Explicitly out of scope: replacing the webview-eval-bridge architecture itself.

### 2.6 — Transactionalize cleanup routines, batch download-queue inserts

`src-tauri/src/commands/system.rs` (`cleanup_duplicates` 298-362, `cleanup_poisoned_artifacts`
364-405), `src-tauri/src/commands/records.rs` (`download_missing_records` 142-215): all three
currently do per-row un-transactioned loops, run unconditionally after every sync. Wrap each in
`pool.begin()`/`tx.commit()`, reusing the exact pattern established in `library/mod.rs`'s
`attach_artifact` (239-296) and in 1.1 — don't invent a new pattern.

### 2.7 — `LIKE lower(...)` predicates can't use indexes (optional, only if time permits)

`src-tauri/src/db/records.rs` (`list` 51-60, `list_page` 109-118): the leading-wildcard
`lower(col) LIKE '%'||lower(?)||'%'` predicate can't use a B-tree index regardless of what indexes
exist — migration `20260526000000_perf_indexes.sql`'s stated goal isn't actually delivered. Real
fix is an FTS5 virtual table (the codebase already has `analysis_chunks_fts` as a precedent — read
its trigger migration and mirror the pattern, don't invent new trigger syntax) indexing
`title`/`summary`/`agency`/`incident_location`. Higher risk (schema change) than other Phase 2
items — defer past 2.1-2.6 if time-constrained and note it as a known fast-follow.

### 2.8 — List virtualization: deferred to Phase 3

GridView/ListView/IntelCardsView render every record's DOM with no windowing — currently masked
by the 250-record cap, more visible once 2.1 ships. Since Phase 3 already retrofits these same
three files for tokens, do virtualization there rather than touching them twice.

### 2.9 — `index_all_records`/`synthesize_all_records` concurrency inconsistency

`src-tauri/src/analysis/batch_processor.rs`: `index_all_records` (10-90) uses a sequential loop
calling the *same* `analysis.index_record(...)` that `analyze_all_records` (92-237) already calls
via `buffer_unordered(2)` — no apparent reason for the difference. Convert `index_all_records` to
the same `buffer_unordered(2)` pattern (reuse the exact progress-counter/event-emit shape).
Leave `synthesize_all_records` (LLM inference, likely far more resource-intensive per call)
sequential deliberately — don't change it without separately vetting concurrent LLM-inference
resource usage.

### Phase 2 verification

**Automated**: `cargo check && cargo test` (must not break `downloads.rs`'s existing
`append_rejects_out_of_order_offsets`/`existing_part_file_reports_resume_offset`/
`finalize_hashes_streamed_file_without_loading_it_all` tests), `bun run check`.

**Manual (`bun tauri dev`)**:
1. Confirm boot issues each status-related IPC call once, not 2-3x (temporarily instrument via
   `logger.debug` if needed); confirm splash clears without the 15s timeout firing.
2. Type "ufo" quickly into search, confirm only one search settles, no result flicker.
3. From the main Dashboard view, confirm "Load more" is visible and works past 250 records.
4. Download a large (>20MB) Release 3 PDF via the webview path before/after 2.5, across 2-3 file
   sizes — watch specifically for stalls or `offset mismatch` errors from `DownloadPartWriter`'s
   existing guard. This is the highest-risk manual check in the phase.
5. Trigger `download_missing_records` on a fresh sync with 250+ candidates, confirm queueing
   isn't visibly serialized/slow.
6. Run `index_all_records` on a batch and confirm no resource exhaustion now that it's concurrent.

---

## PHASE 3 — UI/UX Unification (Tailwind v4 tokens, full 58-component retrofit)

Goal: real `@theme` token system, a shared `Modal`/`Dialog` base, all 4 existing modals migrated
onto it, then all 58 components retrofit folder-by-folder. Do not start until Phase 2's
`Dashboard`/`GridView`/`IntelCardsView` changes (2.1, deferred 2.8) are merged — Phase 3's
`dashboard/` retrofit builds directly on top of them; doing Phase 3 first means touching those
files twice.

### 3.1 — Build the token system

`src/app.css` currently has color/radius/font/transition tokens but no spacing, shadow, or
typography scale, and zero `@theme`/Tailwind usage despite Tailwind v4 being installed.

- First, grep every `.svelte` file for hex literals, `font-size:`, `border-radius:`, and
  `box-shadow:` values to get the actual distinct-value inventory — this is the input to the
  scale, not a from-scratch design exercise. Known worst offenders already identified: `#e7c46b`
  in `Map.svelte` (≡ existing `--accent-primary`), `#3296ff` in Grid/IntelCards views (no token),
  `Logo.svelte`'s disjoint cyan/slate palette, three different "error red"s (`--accent-danger`
  `#f34d4d`, `DownloadAgent.svelte`'s `#ff4646`, settings components' `#ff4d4d`), `#eac`
  independently reinvented in 4 files, 15 distinct font-size values, `DossierSidebar.svelte`'s 5
  ad hoc border-radii.
- Add `@import "tailwindcss"; @theme { ... }` to `app.css`, porting existing color tokens to
  `--color-*` naming for Tailwind utility generation, and adding new `--spacing-*`, `--text-*`,
  `--shadow-*` scales derived from the audit's most-frequent existing values (to minimize visual
  churn) rather than invented numbers.
- Keep the existing `:root { --accent-primary: var(--color-accent-primary); ... }` as an alias
  bridge so components not yet retrofitted keep working unchanged during the incremental
  migration. Remove the bridge only in the final cleanup step (3.6).
- No component changes in this step.

### 3.2 — Legacy store/logger cleanup (independent, do alongside 3.1)

- Convert `src/lib/toastStore.ts` from Svelte-4 `writable()` to the rune-class singleton pattern
  used by the other 8 stores (`class ToastStore { toasts = $state<Toast[]>([]); ... }`), moving it
  to `src/lib/stores/toastStore.svelte.ts`. Update all 7 importers and `Toasts.svelte`'s template
  usage.
- Convert `logger.ts`'s `writable(false)` debug flag to a plain module-level `let` — nothing
  subscribes to it reactively, so this is a simplification, not a rune conversion.
- Zero visual impact; verify via `bun run check` + manual toast smoke test.

### 3.3 — Shared `Modal.svelte` base, migrate the 4 existing modals

New `src/lib/components/Modal.svelte` (flat, matching the existing top-level convention).

- Own: Escape-key close (reuse `MediaViewer.svelte`'s existing `<svelte:window onkeydown>`
  pattern, lines 49-51/79 — the most complete existing implementation), click-outside-dismiss
  (reuse the `onclick={(e) => e.target === e.currentTarget && close()}` pattern from
  `MediaViewer.svelte:85`/`GlobalSearch.svelte:58-60`), a consistent close button (reuse
  `MediaViewer.svelte:120-122`'s markup/class naming). Extract the CSS that's already
  near-line-for-line duplicated between `AnalysisModal.svelte` and `IntelligenceModal.svelte`
  (`.modal-overlay`, panel sizing, `.panel-footer .notice`).
- Props: bindable `isOpen`, matching the existing `bind:isOpen` pattern at all current call sites;
  no separate `onClose` callback needed since consumers already just set `isOpen=false`.
  `GlobalSearch` is store-driven (`appStore.globalSearchOpen`) rather than parent-controlled —
  verify `Modal.svelte`'s API accommodates both during implementation.
- Migrate in this order (smallest/most isolated first, so `Modal.svelte`'s API is proven before
  the harder cases): `IntelligenceModal.svelte` (no close button today, smallest) →
  `AnalysisModal.svelte` (structurally near-identical, should be mechanical) →
  `MediaViewer.svelte` (only the modal *chrome* moves; zoom/pan/rotate state stays as slotted
  content) → `GlobalSearch.svelte` (most different open/close wiring, migrate last). Each
  migration is its own reviewable commit.

### 3.4 — `EmptyState.svelte` decision

`src/lib/components/dashboard/EmptyState.svelte` is currently orphaned (zero imports);
`GridView`/`ListView`/`IntelCardsView` each have their own inline empty-state markup instead.
Read `EmptyState.svelte`'s current props first — if it's not already generic enough to cover
"no records," "no search matches," and "no local records" cases, extend it, then wire it into all
three views as part of the `dashboard/` retrofit group (3.5.3), replacing the duplicated inline
blocks.

### 3.5 — Folder-by-folder retrofit (main body, one PR-sized commit per group)

Per-component checklist: replace hardcoded hex with `@theme` tokens (prefer Tailwind utilities
over hand-rolled `var(--...)` where a clean utility exists; keep scoped `<style>` only for
animations/gradients/pseudo-elements that don't map to utilities), raw font-sizes → `--text-*`,
ad hoc radii → `--radius-*`, hand-authored shadows → `--shadow-*`, static inline `style="..."` →
classes (dynamic state-driven inline styles, e.g. `MediaViewer`'s zoom/rotate transforms,
legitimately stay inline).

Order (~12 groups across 58 files):
1. `layout/` (4 files) — small, low-risk, `SystemSplash.svelte` seen on every boot so it's a good
   early validation that the token bridge works. Fix its `#eac` reinvention.
2. `dashboard/` (7 files) — do the `EmptyState` wiring (3.4), extract a shared `RecordCard.svelte`
   consumed by both `GridView`/`IntelCardsView` (eliminating their duplicated card markup/CSS),
   do the list virtualization deferred from Phase 2 (2.8), fix `#3296ff`. Largest/highest-risk
   group — consider splitting dedup+virtualization from the token retrofit into two commits.
3. `dossier/` (9 files) — fix `DossierSidebar.svelte`'s 5 ad hoc radii, `SynthesisTab.svelte`'s
   `#eac`.
4. `intelligence_center/` (4 files) + `IntelligenceCenter.svelte`.
5. `intelligence_modal/` (3 files) — sequence immediately after the `IntelligenceModal` migration
   (3.3), since these are its sub-panels.
6. `analysis_modal/` (2 files) — same rationale, follows `AnalysisModal` migration.
7. `media/` (3 files) — follows `MediaViewer` migration, since these are its slotted content.
8. `first_launch/` (4 files) + `FirstLaunch.svelte`.
9. `settings/` (6 files) — fix `OptimizationSettings.svelte`/`EnvironmentSettings.svelte`'s
   `#ff4d4d` here.
10. Standalone top-level components (`AppDock`, `EvidenceVault`, `DownloadAgent`,
    `ForensicAuditViewer`, `LinkAnalysis`, `Logo`, `Map`, `Settings`, `Toasts`,
    `IntelligenceDossier`, `agent/AssetList`) — batch into 2-3 sub-groups. `Map.svelte`'s 4x
    `#e7c46b` is the most mechanical fix in the whole retrofit (byte-identical to an existing
    token). `Logo.svelte`'s disjoint cyan/slate palette needs a product decision (confirm with the
    user whether it should move onto the amber/gold system or is an intentional distinct
    brand-mark palette) — don't silently change it.
11. `src/routes/+page.svelte`, `+layout.svelte` — do last since it's the composing shell; fix the
    `#50b3ff` bare literal at line 472.

Explicitly out of scope: renaming `first_launch`/`intelligence_center`-style snake_case folders to
match PascalCase files — real but non-urgent, and renaming risks import breakage for zero visual
benefit. Not part of this retrofit.

### 3.6 — Remove the `:root` alias bridge

Once all groups in 3.5 are merged and a repo-wide grep confirms no component references the old
pre-`@theme` custom property names, remove the alias block added in 3.1.

### Phase 3 verification

**Automated**: `bun run check` and `bun run lint` after every folder-group commit. No Rust
changes expected in this phase.

**Manual (`bun tauri dev`), per commit**:
1. After 3.1 (tokens only): confirm the app looks **pixel-identical** to before — any diff here
   means the alias bridge isn't working correctly.
2. After 3.2: exercise a toast loading→success transition (trigger a sync).
3. After each modal migration (3.3): open it, confirm Escape/click-outside/close-button all work,
   compare before/after screenshots for layout regressions.
4. After the `dashboard/` group: confirm Grid/List/Cards all render correctly with the deduped
   card component, confirm empty states for zero-records/zero-search-results/zero-local-only
   cases, confirm smooth scroll with 250+ records via virtualization.
5. Spot-check the named hex fixes: `Map.svelte` accent color unchanged visually (was already the
   token's literal value), `DossierSidebar.svelte` radii now consistent, the three "error red"s
   now render identically.
6. After 3.6 (alias removal): full click-through of every view — a missing token reference after
   removal typically renders as unstyled/black-on-black, easy to spot.
7. Confirm `:root.performance-mode` (app.css:216-234, targets `[class*='modal']`/`[class*=
   'overlay']`/`[class*='card']` by substring) still works — verify `Modal.svelte`'s class names
   still contain `modal`/`overlay` so this existing accessibility feature doesn't silently break.

---

## Cross-phase notes

- Don't start Phase 2 until 1.1 is merged and verified — 2.6 reuses the exact transaction pattern
  established there.
- Don't start Phase 3 until Phase 2's `Dashboard`/`GridView`/`IntelCardsView` changes are merged —
  Phase 3's `dashboard/` retrofit builds directly on them.
- Bump `package.json`/`src-tauri/Cargo.toml` version per the project's existing release convention
  at the end of each phase (not a technical requirement of this plan, just don't forget it).

## Critical files

- `src-tauri/src/sources/war_gov.rs` — sync/parse/diff core (Phase 1)
- `src-tauri/src/commands/records.rs` — Tauri commands, download orchestration (Phase 1 & 2)
- `src-tauri/src/downloads.rs` — chunked download writer (Phase 2)
- `src/routes/+page.svelte` — sync trigger, boot sequence, dashboard shell (all phases)
- `src/lib/warGovSource.ts` — CSV discovery/validation (Phase 1)
- `src/app.css` — design tokens (Phase 3)
