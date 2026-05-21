# Windows Compatibility & Integration Audit Report - PURSUE Data Analyzer

**Audit Date:** May 21, 2026
**Status:** **CRITICAL ISSUES IDENTIFIED**

## 1. Executive Summary
The PURSUE Data Analyzer has basic support for Windows via its Tauri 2.0 architecture and cross-platform Rust/Python components. However, several critical integration gaps exist that will cause failures or poor UX on Windows systems, particularly regarding window management, Python sidecar initialization, and filesystem robustness.

## 2. Critical Findings

### 2.1. Window Management & UX (Critical)
*   **Undraggable Window:** The application specifies `"decorations": false` and `"transparent": true` in `tauri.conf.json`, but does not provide a `data-tauri-drag-region` in any Svelte components. On Windows, this results in an undraggable window that cannot be moved or resized easily.
*   **Lack of Shadow/Borders:** On Windows, disabling decorations often removes the native drop shadow. The current CSS/HTML does not compensate for this, making the dark application blend into other dark windows or the desktop.

### 2.2. Python Sidecar Environment (High)
*   **Command Name Mismatch:** `sidecar.rs` explicitly uses `Command::new("python3")` to create virtual environments. On standard Windows Python installations, the executable is typically named `python.exe`, and `python3` will result in a "Command not found" error.
*   **Fragile CWD Assumption:** The development sidecar logic uses `std::env::current_dir()?.parent().unwrap().join("src-python")`. This assumes the command is run from `src-tauri`. If run from the workspace root (standard for `bun tauri dev`), this path will be incorrect (`../src-python` vs `./src-python`).

### 2.3. Filesystem & Database Robustness (High)
*   **Factory Reset Failures:** `commands/system.rs` implements `factory_reset` using `std::fs::remove_dir_all` on the app data directory. On Windows, this will fail if the log file (managed by `tauri-plugin-log`) or the database files are still locked by the process.
*   **Database Quarantining:** `db/mod.rs` uses `fs::rename` to move incompatible databases. Windows is much stricter about file locks than macOS/Linux; renaming files while the process still holds a handle (even if the pool is "closed") often triggers `Access Denied`.

### 2.4. Hardware Acceleration (Medium)
*   **Missing DirectML Support:** The `Cargo.toml` and `IntelligenceExtractor` only support CUDA (NVIDIA) and Metal (macOS). Windows users with AMD, Intel, or older NVIDIA GPUs are forced to use CPU inference, even though DirectML could provide acceleration.

## 3. Recommended Remediation Path

### Phase 1: Immediate Stability Fixes
1.  **Fix Python Sidecar:** Update `sidecar.rs` to check for both `python` and `python3` and improve CWD path resolution.
2.  **Enable Window Dragging:** Add `data-tauri-drag-region` to the `AppDock.svelte` and `Logo.svelte` components.

### Phase 2: Windows UX Polishing
1.  **Add Window Shadows:** Implement `window-vibrancy` or use Tauri 2.0's native shadow support for Windows (e.g., via `window_shadows` crate or manual FFI).
2.  **Robust Factory Reset:** Implement a "deferred delete" or "mark for deletion" strategy for the app data directory on Windows to avoid lock contention.

### Phase 3: Performance & Capabilities
1.  **DirectML Integration:** Add `directml` features to `ort` and `candle` dependencies to support non-NVIDIA GPUs on Windows.
