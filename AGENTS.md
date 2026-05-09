# Repository Guidelines

## Project Structure & Module Organization

PURSUE Data Analyzer is a Tauri v2 desktop app with a Svelte 5 frontend and Rust backend.

- `src/routes/`: SvelteKit route entry points (`+page.svelte`, `+layout.svelte`, `+layout.ts`).
- `src/lib/components/`: Reusable UI components such as `Map.svelte` and `ArchiveViewer.svelte`.
- `src/lib/types.ts`: Shared TypeScript interfaces used by the frontend.
- `src/app.css` and `static/`: Global styles and static assets.
- `src-tauri/src/`: Rust application core, including database, ingestion, library, and model modules.
- `src-tauri/migrations/`: Versioned SQLite schema migrations.
- `src-tauri/capabilities/` and `src-tauri/tauri.conf.json`: Tauri permissions and app configuration.

## Build, Test, and Development Commands

Use Bun as the JavaScript package manager; `bun.lock` is the dependency lockfile.

- `bun install`: Install frontend and Tauri CLI dependencies.
- `bun run dev`: Run the Vite/Svelte dev server only.
- `bun tauri dev`: Run the full desktop app with the Rust backend.
- `bun run check`: Sync SvelteKit metadata and run `svelte-check`.
- `bun run build`: Build the static frontend output.
- `cd src-tauri && cargo check`: Type-check the Rust backend.
- `cd src-tauri && cargo test`: Run Rust tests when present.

## Coding Style & Naming Conventions

Use TypeScript for Svelte scripts (`<script lang="ts">`) and keep shared shapes in `src/lib/types.ts`. Components use PascalCase filenames (`ArchiveViewer.svelte`); route files follow SvelteKit naming. Prefer two-space indentation in Svelte/TypeScript and `rustfmt` defaults in Rust. Rust modules and files use snake_case. Keep Tauri commands small and route database or ingestion work through modules under `src-tauri/src/`.

## Testing Guidelines

There is no dedicated frontend test runner configured yet, so `bun run check` is the required frontend validation gate. For backend changes, run `cargo check` and `cargo test` from `src-tauri`. Add Rust unit tests near the module they cover using `#[cfg(test)]`. If adding frontend tests later, place specs beside the component or route they exercise and document the new command in `package.json`.

## Commit & Pull Request Guidelines

Recent commits use short, imperative summaries such as `Add MIT license` and `Update .gitignore with comprehensive Mac-specific entries and build directory`. Keep commits focused and describe the visible change. Pull requests should include a brief purpose statement, testing performed, screenshots for UI changes, and notes for schema, capability, or data-ingestion changes. Link related issues when available.

## Security & Configuration Tips

Treat Tauri capabilities as part of the security boundary. Do not broaden permissions in `src-tauri/capabilities/default.json` without explaining why. Avoid committing databases, indexed evidence, downloaded archives, or generated build outputs.
