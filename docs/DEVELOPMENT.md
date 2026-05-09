# Development

PURSUE Data Analyzer is a Tauri v2 desktop app with a Svelte 5 frontend, Rust backend, and SQLite storage.

## Toolchain

- Bun is the JavaScript package manager. `bun.lock` is the only JavaScript lockfile.
- Node.js 24 or newer is supported for local tooling; GitHub Actions runs Node 26.
- Rust stable is used for the Tauri backend.
- macOS release builds target Apple Silicon only and set `MACOSX_DEPLOYMENT_TARGET=26.0`.

Install the baseline tools:

```bash
brew install bun rustup
rustup default stable
```

Install optional local OCR tools:

```bash
brew install tesseract ocrmypdf poppler
```

## Commands

Install dependencies:

```bash
bun install
```

Run the frontend:

```bash
bun run dev
```

Run the desktop app:

```bash
bun tauri dev
```

Run frontend validation:

```bash
bun run check
bun run build
```

Run backend validation:

```bash
cd src-tauri
cargo check
cargo test
```

Build a local macOS Apple Silicon installer:

```bash
MACOSX_DEPLOYMENT_TARGET=26.0 bun tauri build --target aarch64-apple-darwin
```

## Data Directories

The app stores runtime data under the operating system app data directory:

- `pursue.db`: SQLite database.
- `library/`: SHA-256 content-addressed evidence files.
- `snapshots/`: immutable official source snapshots.
- `exports/`: generated case dossiers.

Do not commit databases, evidence downloads, generated exports, build outputs, or local command captures.

## Tauri Security Boundary

Tauri permissions live in `src-tauri/capabilities/default.json`. Treat capability changes as security-sensitive and explain any broadened permissions in the pull request or commit.

Backend work should keep Tauri commands small and route durable behavior through modules under `src-tauri/src/`.
