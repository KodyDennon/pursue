# PURSUE Data Analyzer

[![Installer builds](https://github.com/KodyDennon/pursue/actions/workflows/release.yml/badge.svg)](https://github.com/KodyDennon/pursue/actions/workflows/release.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

PURSUE Data Analyzer is a local-first desktop OSINT app for syncing, preserving, analyzing, searching, and exporting WAR.gov UFO/PURSUE evidence.

The app keeps official source snapshots, tracks changes between source releases, downloads evidence into a SHA-256 content-addressed local library, imports manual files, extracts local text/OCR when native tools are installed, indexes deterministic entities and local vector chunks, supports case notes, and exports Markdown or self-contained HTML dossiers.

## Download

Installers are published from GitHub Releases:

https://github.com/KodyDennon/pursue/releases/latest

Supported release targets:

- macOS 26 or newer on Apple Silicon (`aarch64-apple-darwin`).
- Windows x64 through the default Tauri Windows installer target.

Release artifacts are unsigned unless signing and notarization secrets are configured in the repository. Unsigned macOS and Windows builds can trigger operating-system warnings.

## Features

- Sync official WAR.gov UFO/PURSUE CSV data with a real user agent.
- Preserve immutable raw source snapshots and added/changed/removed diffs.
- Download official evidence files and deduplicate local artifacts by SHA-256.
- Import investigator-provided local evidence.
- Extract digital PDF text, plain text, image OCR, and scanned-PDF OCR through local tools.
- Index chunks, entities, metadata, and deterministic local vectors in SQLite.
- Search records and analyzed content without hosted APIs.
- Build cases with notes and selected records.
- Export portable Markdown and self-contained HTML dossiers.

## Privacy And Data Boundaries

PURSUE Data Analyzer is local-first. App data is stored under the operating system app data directory and includes `pursue.db`, `library/`, `snapshots/`, and `exports/`.

Network access is used for official WAR.gov source sync and evidence downloads. The app does not require hosted OCR, hosted embeddings, paid AI APIs, or third-party inference services.

## Requirements

For development:

- Bun 1.3.9 or newer.
- Node.js 24 LTS or newer, with CI pinned to Node 26.
- Rust stable.
- Platform build tools for Tauri.

Optional local OCR tools:

```bash
brew install tesseract ocrmypdf poppler
```

Windows builds can run source sync, downloads, imports, digital text extraction, search, cases, and exports without hosted services. Image/scanned-PDF OCR requires local OCR tools available on the Windows machine.

## Development

Install dependencies:

```bash
bun install
```

Run the frontend-only dev server:

```bash
bun run dev
```

Run the full desktop app:

```bash
bun tauri dev
```

Validation gates:

```bash
bun run check
bun run build
cd src-tauri && cargo check
cd src-tauri && cargo test
```

More details are in [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md).

## Releases

The release workflow verifies the frontend and Rust backend, then builds installers for macOS 26 Apple Silicon and Windows. Tags matching `v*` publish non-draft GitHub Releases with downloadable installer assets.

Release documentation is in [docs/RELEASES.md](docs/RELEASES.md).

## Project Layout

- `src/routes/`: SvelteKit route entry points.
- `src/lib/components/`: reusable Svelte UI components.
- `src/lib/types.ts`: shared frontend TypeScript shapes.
- `src-tauri/src/`: Rust application core.
- `src-tauri/migrations/`: SQLite schema migrations.
- `src-tauri/capabilities/`: Tauri permissions.
- `.github/workflows/`: verification, installer, and release automation.

## Status

Current implementation status is tracked in [PROJECT_STATUS.md](PROJECT_STATUS.md). Product and data contracts are tracked in [PURSUE_BLUEPRINT.md](PURSUE_BLUEPRINT.md).

## License

MIT. See [LICENSE](LICENSE).
