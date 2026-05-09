# PURSUE Data Analyzer

PURSUE Data Analyzer is a local-first Tauri v2 desktop app for syncing, preserving, analyzing, searching, and exporting WAR.gov UFO/PURSUE evidence.

The app stores official source snapshots, records diffs between rolling releases, downloads evidence into a SHA-256 content-addressed local library, imports manual evidence, extracts local text/OCR where native tools are installed, indexes deterministic entities and local vector chunks, supports cases and notes, and exports Markdown or self-contained HTML dossiers.

## Stack

- Tauri v2 desktop shell with Rust backend
- Svelte 5 frontend with Bun
- SQLite via SQLx migrations
- Local files under the app data directory: `pursue.db`, `library/`, `snapshots/`, and `exports/`
- Local OCR tools only: no API keys, hosted AI, hosted OCR, or paid services

## Setup

Install the required development tools:

```bash
brew install bun rustup
rustup default stable
```

Install optional local analysis tools:

```bash
brew install tesseract ocrmypdf poppler
```

Windows builds require Rust, Bun, WebView2, and the Windows installers for Tesseract and OCRmyPDF if image/scanned-PDF OCR is needed. Digital PDF/text extraction, source sync, downloads, cases, search over indexed text, and exports do not require hosted services.

Install dependencies:

```bash
bun install
```

## Development

```bash
bun run dev
bun tauri dev
```

Use `bun run dev` for the Vite/Svelte dev server only. Use `bun tauri dev` for the full desktop app with the Rust backend and SQLite app data.

## Verification

```bash
bun run check
bun run build
cd src-tauri && cargo check
cd src-tauri && cargo test
```

## Implemented Commands

- `sync_official_source`
- `list_records`
- `download_record`
- `download_missing_records`
- `get_bulk_download_status`
- `cancel_bulk_download`
- `import_manual_file`
- `analyze_record`
- `get_analysis_result`
- `search`
- `list_cases`
- `create_case`
- `update_case_notes`
- `add_record_to_case`
- `export_case`

## Data Integrity

Every ingested artifact is streamed or copied through SHA-256 hashing before it is committed to the managed library. Repeated downloads/imports deduplicate by hash. Official syncs write immutable raw CSV snapshots and compute added, changed, and removed records against the previous completed snapshot.

## Installer Builds

GitHub Actions runs Svelte checks, frontend build, Rust check/test, and Tauri installer builds for macOS and Windows. Release builds are unsigned unless signing/notarization secrets are configured in the repository.

Unsigned macOS and Windows builds may show operating-system warnings. Signed builds use the same workflow when `APPLE_*` or `TAURI_SIGNING_*` secrets are present.
