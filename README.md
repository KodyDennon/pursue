# PURSUE Data Analyzer

[![Installer builds](https://github.com/KodyDennon/pursue/actions/workflows/release.yml/badge.svg)](https://github.com/KodyDennon/pursue/actions/workflows/release.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

PURSUE Data Analyzer is a local-first desktop OSINT app for syncing, preserving, analyzing, searching, and exporting WAR.gov UFO/PURSUE evidence.

The app keeps official source snapshots, tracks changes between source releases, downloads evidence into a SHA-256 content-addressed local library, imports manual files, extracts local text/OCR when native tools are installed, indexes deterministic entities and local vector chunks, supports case notes, and exports Markdown or self-contained HTML dossiers.

## Download

The machine-aware download center includes system selection, install guidance, privacy details, and current SHA-256 checksums:

https://downloads.kodydennon.com/PURSUE

Installers are mirrored through Cloudflare R2 after their GitHub digests are verified. GitHub Releases remains the fallback, authoritative history, and source of truth:

https://github.com/KodyDennon/pursue/releases/latest

The production mirror setup, two-version retention policy, and integrity contract are documented in [docs/R2_MIRROR_HANDOFF.md](docs/R2_MIRROR_HANDOFF.md).

The private multi-project portal owns shared presentation and deployment, while this repository owns PURSUE's validated public copy in [`downloads/project.json`](downloads/project.json). Editing, download-count privacy, SEO/answer-discovery, and repository responsibility boundaries are documented in [docs/DOWNLOADS_HUB.md](docs/DOWNLOADS_HUB.md).

Supported production lanes:

- macOS 26 or newer on Apple Silicon, with Metal/CoreML acceleration.
- Windows x64 CUDA for NVIDIA GPUs (Turing/SM75 and newer).
- Windows x64 DirectML for other supported Windows GPUs.

Automatic updater bundles and signing are currently disabled. Download the latest installer for your platform when updating; ordinary reinstalls preserve PURSUE data. The installers are also not Authenticode-signed or Apple-notarized, so Windows SmartScreen and macOS Gatekeeper can still warn about the installer/application.

### macOS Install Notes

Download the release's `PURSUE.Data.Analyzer_<version>_aarch64.dmg`, open it, and drag `PURSUE Data Analyzer.app` to `/Applications`.

Because the current app is not Apple-notarized, macOS may say it cannot be opened. If you trust the official release, remove the quarantine attribute after installing it:

```bash
xattr -dr com.apple.quarantine "/Applications/PURSUE Data Analyzer.app"
```

Then open the app again from Finder or Spotlight.

Alternative macOS path:

1. Open System Settings.
2. Go to Privacy & Security.
3. If macOS shows a blocked app message for PURSUE Data Analyzer, choose Open Anyway.

### Windows Install Notes

Download one Windows installer from the release:

- `PURSUE.Data.Analyzer_<version>_x64-cuda_en-US.msi`: self-contained NVIDIA CUDA installer; preferred for supported NVIDIA GPUs. CUDA is MSI-only because its bundled runtime payload exceeds NSIS's 2 GiB compiler limit.
- `PURSUE.Data.Analyzer_<version>_x64-setup.exe` and matching `.msi`: DirectML installers for other Windows GPUs.

Windows may show a Microsoft Defender SmartScreen warning until Windows code signing is configured. Choose More info, then Run anyway only for an official release.

The installer preserves the database, evidence vault, model cache, exports, resumable downloads, and custom-storage pointer across updates/uninstall/reinstall. It bundles Microsoft's signed Visual C++ redistributable and can bootstrap WebView2 when missing. CUDA builds ship the required CUDA/cuDNN/ONNX runtime DLLs; users do not need a CUDA toolkit.

## Features

- Sync official WAR.gov UFO/PURSUE CSV data with a real user agent.
- Preserve immutable raw source snapshots and added/changed/removed diffs.
- Download official evidence files and deduplicate local artifacts by SHA-256.
- Import investigator-provided local evidence.
- Extract digital PDF text, plain text, and scanned/image OCR through bundled local models.
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

Production OCR models are bundled. Gemma 4 E4B and gated Hugging Face assets are provisioned by onboarding; Hugging Face device authorization opens a browser without asking users to paste access tokens into the app. CPU inference is the final fallback after CUDA, CoreML, and DirectML provider attempts.

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

The release workflow verifies the frontend and Rust backend, then builds unsigned installers for macOS 26+ Apple Silicon, Windows DirectML, and Windows CUDA. Tags matching `v*` publish non-draft GitHub Releases with downloadable installer assets.

Each production tag publishes four installer files. Paid OS code signing and Tauri updater signing are disabled; users update by downloading the new installer. If R2 mirroring is enabled, the exact installer bytes and digests are copied to immutable R2 keys before stable aliases are advanced. Once the new mirror is publicly verified, R2 keeps only the current and immediately previous immutable release prefixes.

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
