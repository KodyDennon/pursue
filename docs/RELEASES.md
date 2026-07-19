# Production Releases

PURSUE releases are built, tested, signed for in-app updates, and published by GitHub Actions. GitHub Releases is the source of truth; an optional Cloudflare R2 mirror provides an independent large-file origin.

## Release lanes

- `macos-metal-aarch64`: macOS 14+ on Apple Silicon using Metal/CoreML.
- `windows-cuda-x86_64`: Windows x64 with bundled CUDA 12/cuDNN/ONNX runtimes for NVIDIA Turing (SM75) and newer.
- `windows-directml-x86_64`: Windows x64 using DirectML for non-NVIDIA or CUDA-incompatible GPUs.

There is no Intel or universal macOS artifact. Windows users do not need a CUDA toolkit: the CUDA installer carries the redistributable runtime libraries. Both Windows installers carry Microsoft's signed Visual C++ redistributable and can bootstrap WebView2 when it is missing.

## Update and persistence contract

The application checks a signed `latest.json` manifest after startup and on demand. It requests only its exact acceleration lane, verifies the Tauri updater signature before installation, runs SQLite `quick_check`, checkpoints the WAL, installs, and relaunches.

An ordinary update, uninstall, or reinstall must preserve:

- `pursue.db`, WAL state, and versioned pre-migration backups;
- the evidence library and source snapshots;
- model files and resumable model/evidence downloads;
- exports and the custom-storage pointer;
- user settings and credentials stored by the OS keychain.

Destructive data removal is available only from the explicit Factory Reset flow inside the app.

The updater signature is not Windows Authenticode signing or Apple Developer ID notarization. Those OS trust systems require separate certificates; until configured, SmartScreen or Gatekeeper can still warn even though the in-app update bundle is cryptographically verified.

## Required GitHub configuration

Encrypted repository secrets:

- `TAURI_SIGNING_PRIVATE_KEY`
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`

The matching public key is committed at `plugins.updater.pubkey` in `src-tauri/tauri.conf.json`. Never store the private key in a GitHub variable, repository file, workflow output, or release asset.

Optional R2 variables and secrets are documented in `docs/R2_MIRROR_HANDOFF.md`.

## Validation performed by `build-installers`

Every relevant push or pull request runs:

- frozen Bun install, Svelte typecheck, frontend production build, and lint-compatible configuration;
- macOS Metal `cargo check` and tests;
- Windows DirectML `cargo check` and tests with the production manifest embedded in the test executable;
- installer compilation for Apple Silicon Metal, Windows DirectML, and Windows CUDA;
- pinned PDFium and native-runtime staging checks.

Tag builds additionally require the signing secrets, produce Tauri updater artifacts, publish installers and signatures, generate a strict three-lane `latest.json`, and mirror to R2 when enabled. The manifest job refuses missing, duplicate, partial, draft, or non-HTTPS release assets.

## Published assets

A production tag is complete only when GitHub contains:

- one Apple Silicon DMG;
- one Windows CUDA setup EXE and MSI;
- one Windows DirectML setup EXE and MSI;
- one Apple Silicon `.app.tar.gz` updater and `.sig`;
- one CUDA `.msi.zip` updater and `.sig` (the self-contained CUDA payload exceeds NSIS's 2 GiB compiler limit);
- one DirectML `.nsis.zip` updater and `.sig`;
- `latest.json` containing exactly the three release lanes.

## Publishing

The `Auto Version and Release` workflow accepts commit subjects beginning with:

- `release: patch`
- `release: minor`
- `release: major`
- `release: 1.2.3`

It synchronizes `package.json`, `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`, and `src-tauri/tauri.conf.json`, then creates the matching `v*` tag. A manually created tag is also supported, but all four version sources must already agree.

After pushing, monitor both `Auto Version and Release` and `build-installers` to completion. Do not announce production deployment until the tag exists, all required jobs are green, the GitHub asset inventory is complete, updater signatures match their manifests, and any enabled R2 mirror has passed public URL/digest verification.
