# Releasing PURSUE

Releases are automated except for the Windows **CUDA** installer, which is built on the
maintainer's machine. This split exists because the CI CUDA installer job took **~2h20m**
(≈94% of the whole build): GitHub's runners recompile the vendored CUDA/llama kernels cold
every run, while a local machine with a warm `src-tauri/target` does the same build
incrementally in a few minutes. macOS and DirectML stay in CI (they finish in ~12–22 min and
macOS cannot be built on Windows).

Installers are unsigned (`--no-sign`), so a locally built MSI is byte-for-purpose equivalent
to a CI one. The downloads portal ([R2 mirror](.github/scripts/mirror-release-to-r2.sh))
requires **exactly four production installers** on the GitHub release before it publishes:

| Alias (portal) | Installer name pattern | Built by |
| --- | --- | --- |
| `macos-apple-silicon.dmg` | `*_aarch64.dmg` | CI |
| `windows-directml.msi` | `*_x64_en-US.msi` | CI |
| `windows-directml-setup.exe` | `*_x64-setup.exe` | CI |
| `windows-cuda.msi` | `*_x64-cuda_en-US.msi` | **local** |

## One-time prerequisites (maintainer machine, Windows)

Already required and typically present: **Rust**, **bun**, **Node 24+**, **gh** (authenticated),
**Visual Studio 2022** (or Build Tools) with the C++ toolset, **CMake** + **Ninja** (VS bundles
them; the script finds them across VS installs and adds them to `PATH`), and **libclang**
(`libclang.dll`) — either a standalone LLVM or the repo's `.tools\llvm\bin`.

**CUDA Toolkit 12.4–12.9** (not 12.0/12.1/12.2/12.3) with `CUDA_PATH` pointing at it. The
vendored `candle-kernels` use the `__hmax_nan`/`__hmin_nan` half intrinsics and headers that the
current MSVC STL gates behind *"CUDA 12.4 or newer"*, so **CUDA < 12.4 fails to compile the
kernels**. Stay on the 12.x line so the app ships `cudart64_12.dll` (the staging script and CI
target CUDA 12; CI uses 12.4.1). A modern MSVC toolset (14.4x) is fine — `cudaforge` passes
`-allow-unsupported-compiler` to nvcc. cuDNN is downloaded automatically (pinned) by the staging
script. The script fails fast with a clear message if `CUDA_PATH` points at a toolkit older than
12.4.

## Cutting a release

1. **Trigger the version bump + tag.** Commit to `main` with a message starting `release:`
   (`release: patch` | `minor` | `major`, or `release: X.Y.Z`). `auto-release.yml` bumps
   `package.json` / `Cargo.toml` / `tauri.conf.json`, commits `chore: release vX.Y.Z`, pushes
   the tag, and dispatches `build-installers`.

2. **Let CI build the three fast installers.** `build-installers` (release.yml) produces the
   macOS dmg + DirectML msi/exe and uploads them to the `vX.Y.Z` release (~25 min).

3. **Build + publish the CUDA installer locally.** After pulling the version-bump commit:

   ```powershell
   git pull
   pwsh scripts/release-cuda-local.ps1
   ```

   The script reproduces the CI CUDA job (frontend build → PDFium/CUDA/cuDNN runtime staging
   → `tauri build --no-sign --bundles msi`), renames the MSI to
   `PURSUE.Data.Analyzer_<version>_x64-cuda_en-US.msi`, `gh release upload`s it, and dispatches
   `mirror-release.yml`. The mirror verifies all four installers, uploads them to R2, and
   refreshes the portal manifest.

   Useful flags: `-Tag vX.Y.Z` (target a specific release), `-CancelCiCudaJob` (cancel a
   still-running CI CUDA job left over from an older workflow), `-SkipMirror` (upload only),
   `-SkipUpload` (build + rename only). The script prints a per-step timing summary.

## Notes / failure modes

- If you skip step 3, the release has only three installers and the mirror **fails loudly** —
  the portal keeps serving the previous version (fails safe). Re-run step 3 to complete it.
- CUDA build correctness is still checked in CI by `cuda-check.yml` on pushes/PRs.
- `cargo test` does not run locally on the maintainer's Windows machine (a pre-existing loader
  issue); CI runs the Rust tests on macOS.
