# Production Releases

PURSUE releases are built, tested, and published by GitHub Actions. GitHub Releases is the source of truth; an optional Cloudflare R2 mirror provides an independent large-file origin.

## Release lanes

- `macos-metal-aarch64`: macOS 26+ on Apple Silicon using Metal/CoreML.
- `windows-cuda-x86_64`: Windows x64 with bundled CUDA 12/cuDNN/ONNX runtimes for NVIDIA Turing (SM75) and newer.
- `windows-directml-x86_64`: Windows x64 using DirectML for non-NVIDIA or CUDA-incompatible GPUs.

There is no Intel or universal macOS artifact. Windows users do not need a CUDA toolkit: the CUDA installer carries the redistributable runtime libraries. Both Windows installers carry Microsoft's signed Visual C++ redistributable and can bootstrap WebView2 when it is missing.

## Update and persistence contract

Updater bundle generation and signing are disabled. Users update by downloading and running the latest installer from GitHub Releases or the R2 mirror. The installer and application preserve user data across ordinary updates.

An ordinary update, uninstall, or reinstall must preserve:

- `pursue.db`, WAL state, and versioned pre-migration backups;
- the evidence library and source snapshots;
- model files and resumable model/evidence downloads;
- exports and the custom-storage pointer;
- user settings and credentials stored by the OS keychain.

Destructive data removal is available only from the explicit Factory Reset flow inside the app.

Windows Authenticode and Apple Developer ID notarization are also disabled, so SmartScreen or Gatekeeper can warn. No paid code-signing certificate or updater-signing secret is required by the release workflow.

Optional R2 variables and secrets are documented in `docs/R2_MIRROR_HANDOFF.md`.

## Validation performed by `build-installers`

Every relevant push or pull request runs:

- frozen Bun install, Svelte typecheck, frontend production build, and lint-compatible configuration;
- macOS Metal `cargo check` and tests;
- Windows DirectML `cargo check` and tests with the production manifest embedded in the test executable;
- installer compilation for Apple Silicon Metal, Windows DirectML, and Windows CUDA;
- pinned PDFium and native-runtime staging checks.

Tag builds additionally publish the four unsigned installers and mirror them to R2 when enabled. The mirror refuses missing, duplicate, partial, draft, or non-HTTPS release assets. After public verification, it prunes immutable release prefixes beyond the current and immediately previous versions.

## Published assets

A production tag is complete only when GitHub contains:

- one Apple Silicon DMG;
- one Windows CUDA MSI (the self-contained CUDA payload exceeds NSIS's 2 GiB compiler limit);
- one Windows DirectML setup EXE and MSI.

## Publishing

The `Auto Version and Release` workflow accepts commit subjects beginning with:

- `release: patch`
- `release: minor`
- `release: major`
- `release: 1.2.3`

It synchronizes `package.json`, `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`, and `src-tauri/tauri.conf.json`, then creates the matching `v*` tag. A manually created tag is also supported, but all four version sources must already agree.

After pushing, monitor both `Auto Version and Release` and `build-installers` to completion. Do not announce production deployment until the tag exists, all required jobs are green, the four-file GitHub asset inventory is complete, and any enabled R2 mirror has passed public URL/digest verification.
