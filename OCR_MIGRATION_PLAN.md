# Native OCR & PDF Processing Migration Plan

## Objective
Retire the fragile, resource-heavy Python `GOT-OCR-2.0` sidecar and replace it with a fast, deterministic, Pure Rust + ONNX neural OCR engine. This migration will eliminate external dependencies, reduce startup latency from 30+ seconds to milliseconds, and ensure all PDFs (digital and scanned) are securely processed for redactions using OS-native rendering.

**CRITICAL RULE ENFORCED:** Under no circumstances will OS-native OCR (e.g., Apple Vision Framework, Windows Media OCR) be used, as documented in `GEMINI.md`. All text recognition will be performed by the local ONNX neural network.

## Key Files & Context
*   `src-python/` (Directory to be completely deleted)
*   `src-tauri/src/analysis/python_env.rs` (To be deleted)
*   `src-tauri/src/analysis/sidecar.rs` (To be heavily refactored/removed)
*   `src-tauri/src/analysis/ocr.rs` (To house the new ONNX implementation)
*   `src-tauri/src/analysis/pdf.rs` (To house the OS-native PDF-to-Image rendering logic)
*   `src-tauri/Cargo.toml` (Dependency updates)

## Proposed Solution

1.  **Eliminate the Python Environment:**
    *   Remove all Python installation, pip bootstrapping, and sidecar management logic.
    *   Delete the `src-python` directory and remove references in `tauri.conf.json`.

2.  **Implement Pure Rust ONNX OCR (`oar-ocr`):**
    *   Integrate the `oar-ocr` crate, which uses `ort` (ONNX Runtime) and pure Rust image processing (avoiding OpenCV).
    *   Bundle the lightweight PaddleOCR ONNX models (Detection and Recognition, ~15MB total) directly within the Tauri app assets for immediate, offline availability.
    *   Rewrite `src-tauri/src/analysis/ocr.rs` to initialize the ONNX models and perform inference.

3.  **OS-Native PDF Rendering for Redaction Detection:**
    *   Update `src-tauri/src/analysis/pdf.rs` to render PDF pages to image buffers using OS-native APIs:
        *   **macOS:** `objc2-pdf-kit` and `objc2-core-graphics`.
        *   **Windows:** `windows::Data::Pdf`.
    *   *Workflow:* Every PDF page (even digital ones) will be rendered to an image buffer. This buffer will be passed through the existing `analyze_redactions` function to calculate redaction scores.
    *   *OCR Workflow:* If the digital text extraction via `lopdf` yields insufficient text, the generated image buffers will be passed to the new ONNX OCR engine.

## Implementation Steps

### Phase 1: Cleanup & Dependency Management
1.  Delete `src-python/` directory.
2.  Remove `src-tauri/src/analysis/python_env.rs`.
3.  Remove vestigial 0-byte binaries from `src-tauri/binaries/`.
4.  Update `src-tauri/Cargo.toml`:
    *   Add `oar-ocr` and `ort` (if not fully configured for this use case).
    *   Ensure OS-specific PDF rendering dependencies (`objc2-pdf-kit`, `windows::Data::Pdf`) are correctly specified.
    *   Remove unused `objc2-vision` (enforcing the No Native OCR rule).

### Phase 2: PDF Rendering (OS Native)
1.  Modify `src-tauri/src/analysis/pdf.rs`.
2.  Implement `render_page_to_image(page_num)` utilizing conditional compilation (`#[cfg(target_os = "...")]`) for macOS and Windows.
3.  Ensure the output is a standard image buffer (e.g., `image::DynamicImage`) compatible with the redaction analyzer and OCR engine.

### Phase 3: ONNX OCR Implementation
1.  Procure the standard PaddleOCR ONNX models (det, rec, and dictionary) and place them in a dedicated assets folder (e.g., `src-tauri/assets/models/`).
2.  Rewrite `src-tauri/src/analysis/ocr.rs`:
    *   Initialize the `OAROCRBuilder` with the bundled ONNX models.
    *   Implement `extract_text(&self, image: &DynamicImage) -> Result<String>`.
3.  Refactor `indexer.rs` and `mod.rs` to route extraction requests to the new ONNX engine instead of the old sidecar.

### Phase 4: Integration & Testing
1.  Update the `indexer.rs` logic to ensure all PDFs trigger the redaction pass via the new native rendering logic.
2.  Verify that digital text is preferred for text extraction, but ONNX OCR is invoked for scanned images and pages lacking digital text.
3.  Remove the `VisionSidecar` struct and its initialization from the main application lifecycle.

## Verification & Testing
*   **Startup Latency:** Verify the app starts and is ready to process documents immediately (no 30-second delay).
*   **Redaction Detection:** Process a PDF containing redactions and confirm the redaction score is accurately calculated using the native PDF rendering.
*   **Accuracy:** Process a noisy/scanned image and verify the Pure Rust ONNX implementation extracts text accurately.
*   **Build Size:** Confirm the overall application bundle size is reasonable (~15MB larger for the models, but significantly smaller by removing the Python logic).

## Migration & Rollback
*   If the `oar-ocr` implementation fails to build cross-platform, we will pivot to the `paddle-ocr-rs` (OpenCV) or `ocrs` (Pure Rust RTen) crates as fallbacks.
*   The old Python implementation will be preserved in git history and can be restored if the ONNX accuracy is deemed insufficient, though this is highly unlikely given the models used.