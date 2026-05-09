# PURSUE Data Analyzer Blueprint

## Product Contract

PURSUE Data Analyzer is a local-first OSINT desktop app for WAR.gov UFO/PURSUE evidence and user-imported files. The app is responsible for provenance, local preservation, repeatable analysis, case work, and portable dossier output.

## Implemented Architecture

- `src/routes/` and `src/lib/components/`: Svelte 5 desktop UI.
- `src-tauri/src/db`: SQLite initialization and record queries.
- `src-tauri/src/sources`: official WAR.gov source adapter.
- `src-tauri/src/library`: SHA-256 content-addressed evidence library.
- `src-tauri/src/analysis`: local text/OCR extraction, entity extraction, and chunk indexing.
- `src-tauri/src/search`: SQLite/FTS and deterministic local vector search.
- `src-tauri/src/cases`: cases, case records, and investigator notes.
- `src-tauri/src/exports`: Markdown and self-contained HTML dossier generation.
- `src-tauri/src/commands`: typed Tauri command boundary.

## Data Rules

- Official source snapshots are immutable files under the app data snapshot directory.
- Official records are keyed by source URL when present, with metadata hash fallback when no URL exists.
- Local evidence artifacts are keyed by SHA-256 and stored once.
- Manual imports use `source_type = manual`.
- Removed official records are retained locally and marked with `removed_from_source_at`.
- Case exports include source provenance, local paths, hashes, notes, entities, and analysis excerpts.

## Local Analysis Rules

- Digital PDF text extraction runs first.
- Text-like files are read directly.
- Image OCR uses the local `tesseract` command when installed.
- Scanned-PDF OCR uses the local `ocrmypdf` command when installed.
- Entity extraction is deterministic and stored in SQLite.
- Search is local-only and combines metadata, indexed chunks, SQLite FTS, and deterministic vector scoring.

## Release Rules

- CI verifies frontend checks/build and Rust check/test.
- CI uses Bun 1.3.9, Node 26, Rust stable, and `tauri-apps/tauri-action`.
- macOS installer builds target macOS 26 or newer on Apple Silicon only.
- Windows installer builds target 64-bit Windows through Tauri's default Windows target.
- Tags matching `v*` publish non-draft GitHub Releases with downloadable installer assets.
- `release: patch`, `release: minor`, `release: major`, and `release: x.y.z` commits on `main` can bump app versions and create the matching tag.
- Signing and notarization are automatic only when repository secrets are present.
