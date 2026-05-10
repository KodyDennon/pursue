# Project Status: PURSUE Data Analyzer

Last updated: May 10, 2026

## Implemented

- Tauri v2, Svelte 5, Bun, and SQLite application foundation.
- WAR.gov UFO CSV sync with a real user-agent, raw source snapshots, stable record keys, content hashes, and added/changed/removed diffs.
- Managed evidence library with SHA-256 content-addressed storage, duplicate detection, single-record downloads, bulk missing downloads, cancellation flagging, and manual file import.
- SQLite migrations for source snapshots, diffs, artifacts, download jobs/items, analysis chunks, entities, case notes, intelligence fragments, and forensic reports.
- Local-first Decoupled Intelligence Pipeline (Indexing vs. Synthesis) with Gemma 4 neural engine support.
- Phase 1 (Indexing): Foundation layer with digital text extraction, multi-engine OCR (macOS Vision / Windows Media / Tesseract), and entity graph population.
## Current Status (May 10, 2026)
- **Structural Hardening Complete**: Pipeline decoupled into Indexer/Synthesizer/Persistence services.
- **Async Purity**: All `block_on` anti-patterns eliminated; inference isolated in `spawn_blocking`.
- **Model Manifest**: Centralized `ModelRegistry` (v1.0) synced across Rust and Svelte.
- **Database Baseline**: Squashed migrations into a unified v1.0 baseline with strict Fkey integrity.
- **Neural Auditability**: Real-time "Thought Stream" diagnostic UI deployed in Analysis Modal.

## Core Implementation
- Phase 2 (Synthesis): High-fidelity forensic audit and intelligence synthesis via local Gemma 4 inference with stateful KV-caching.
- Deterministic and neural-augmented entity extraction for agencies, dates, locations, and suspicious patterns.
- Local high-performance vector search via sqlite-vec and ONNX (bge-small); no hosted embeddings or API inference.
- Case creation, case membership, investigator notes, and forensic dossier exports.
- Premium "Black-Ops" tactical UI for intelligence management, geospatial mapping, and automated collection.
- Public GitHub repository metadata, docs, and release automation.
- macOS 26 Apple Silicon and Windows installer workflow using `tauri-apps/tauri-action`.

## Runtime Requirements

- Network is used only for official WAR.gov sync and official evidence downloads.
- OCR requires local native tools. Digital PDF/text analysis works without OCR tools when text is embedded in the artifact.
- macOS release artifacts target macOS 26 or newer on Apple Silicon only.
- Windows release artifacts target 64-bit Windows through Tauri's default Windows target.
- Release artifacts are unsigned by default so publishing is not blocked by invalid signing secrets.

## Verification Gates

- `bun run check`
- `bun run build`
- `cd src-tauri && cargo check`
- `cd src-tauri && cargo test`
- GitHub Actions `build-installers` on `main`, pull requests, manual dispatch, and `v*` tags
