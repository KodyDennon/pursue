# Project Status: PURSUE Data Analyzer

Last updated: May 9, 2026

## Implemented

- Tauri v2, Svelte 5, Bun, and SQLite application foundation.
- WAR.gov UFO CSV sync with a real user-agent, raw source snapshots, stable record keys, content hashes, and added/changed/removed diffs.
- Managed evidence library with SHA-256 content-addressed storage, duplicate detection, single-record downloads, bulk missing downloads, cancellation flagging, and manual file import.
- SQLite migrations for source snapshots, diffs, artifacts, download jobs/items, analysis chunks, entities, case notes, and export records.
- Local analysis pipeline for digital PDFs, text-like files, image OCR through a detected local Tesseract binary, and scanned-PDF OCR through detected local OCRmyPDF.
- Deterministic entity extraction for agencies, dates, locations, file references, object-shape terms, sensors, and person-like names.
- Local chunk indexing with SQLite FTS plus deterministic local vector scoring; no hosted embeddings or API inference.
- Case creation, case membership, investigator notes, and Markdown/HTML dossier exports.
- Desktop UI for sync, import, filters, map, search, downloads, analysis, cases, notes, and exports.
- macOS and Windows installer workflow using `tauri-apps/tauri-action`.

## Runtime Requirements

- Network is used only for official WAR.gov sync and official evidence downloads.
- OCR requires local native tools. Digital PDF/text analysis works without OCR tools when text is embedded in the artifact.
- Release artifacts are unsigned unless signing/notarization secrets are configured.

## Verification Gates

- `bun run check`
- `bun run build`
- `cd src-tauri && cargo check`
- `cd src-tauri && cargo test`
