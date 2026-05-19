# PURSUE Codebase Audit Report (May 18, 2026)

## Overview
This report documents a deep, exhaustive audit of the PURSUE Data Analyzer codebase. The audit identified systemic failures in the Intelligence Pipeline, OCR infrastructure, IPC communication, and UI/UX state management.

---

## 1. Intelligence Pipeline (Neural Engine)

### 1.1 Mathematical Logic Errors (Gemma 4)
- **Activation Function Mismatch**: The implementation in `gemma4.rs` utilizes `gelu_erf()` for activations. The Gemma architecture (especially v2/v4 variants) strictly requires a tanh-approximated GELU (`gelu()`). This discrepancy leads to weight misalignment during inference, resulting in garbage text or "hallucinated" JSON structures.
- **RoPE (Rotary Positional Embedding) Application**: In `Attention::forward`, the rotary embeddings are applied before KV-cache appending, which is correct, but there is no validation that `index` (the position offset) is being tracked correctly across multiple calls during auto-regressive generation in `extraction.rs`.

### 1.2 Inference Orchestration
- **Redundant Processing**: `IntelligenceExtractor::extract_metadata` performs a RAG query for 15 fragments but also pulls the entire `ocr_text`. If the text is over 5000 characters, it slices the *first* 2000 characters but still includes the "Critical Context" fragments. This is suboptimal for large documents where the most relevant data might be in the middle of the text.

---

## 2. OCR & Foundation Indexing

### 2.1 Fatal PDF Processing Bug
- **Crash Path**: In `ocr.rs`, the function `extract_text_fallback` immediately calls `image::open(image_path)`. When a PDF file reaches this fallback (e.g., if native macOS/Windows OCR fails or is not present), the `image` crate attempts to decode a PDF as a pixel format, which results in an immediate `Err` or panic. 
- **Missing PDF-to-Image Pipeline**: There is no logic to render PDF pages into images *before* sending them to the Rust-based OCR engine (`ocrs`).

### 2.2 Ignored "Force OCR" Protocol
- **Logic Bypass**: The `indexer.rs` file accepts a `force_ocr` boolean, but the `extract_pdf` function explicitly ignores it. It defaults to native OCR and then falls back to digital text. If a user requests a "Deep Re-Audit" (Pixel OCR), the system will still just pull the digital text layer if available, failing to bypass corrupted or redacted text layers.

---

## 3. Communication & IPC (Tauri)

### 3.1 Signature Mismatches (Frontend vs. Backend)
- **Command Rejection**: `AnalysisModal.svelte` and `IntelligenceCenter.svelte` call `analyze_all_records` and `reprocess_all_records` passing a JSON object: `{ forceOcr: boolean }`.
- **Rust Implementation**: In `commands/analysis.rs`, these functions are defined as `pub async fn analyze_all_records(state: State, app: AppHandle) -> Result<usize, String>`. They **do not** accept any arguments. 
- **Result**: Tauri fails to deserialize the arguments, and the commands are never executed. The UI remains in "Initializing" or "Standby" forever, or fails silently.

### 3.2 Event Desync
- **Status Name Mismatches**: The backend emits `completed` and `batch-complete`, but the frontend `AnalysisModal.svelte` is looking for `completed` and `record-completed`. 
- **Missing `record_id` in Trace**: Sub-step events emitted from `native_macos.rs` (e.g., "Extracting page 1/5") do not include the `record_id`. If the UI is tracking multiple records or a batch, it cannot correctly attribute these sub-steps to the current unit.

---

## 4. UI/UX Failures

### 4.1 Broken Modals & States
- **Analysis Modal Persistence**: The modal logic for `busy` state relies on the `analysis-progress` event. If the backend fails to start (due to the IPC mismatch), the modal enters an "Initializing" state that cannot be escaped without a refresh.
- **Thought Stream UI**: The `thought-section` in `AnalysisModal.svelte` only displays text if the status is exactly `synthesizing` or `reasoning`. However, the backend emits `synthesizing-start` first, which might cause a flicker or delay in showing the stream.

### 4.2 Intelligence Center Discrepancies
- **Force Re-Audit Trigger**: The "Force Re-Audit" button in `IntelligenceCenter.svelte` passes `{ forceOcr: true }`, but the logic for handling the progress event doesn't properly reset the `processedCount`, leading to "0 of 0" or incorrect progress bars.

---

## 5. Persistence & Database

### 5.1 Redundant SQL Operations
- **Double Inserts**: `AnalysisManager::index_record_inner` contains two identical `INSERT INTO analysis_results` blocks for the same `record_id` within the same function flow. This is token-inefficient and risks deadlocks or unnecessary disk I/O.
- **FKey Integrity**: While migrations are "squashed," some tables like `vec_analysis_chunks` use `chunk_id` as a primary key but do not explicitly have a `ON DELETE CASCADE` relationship at the SQLite `vec0` level (which is a known limitation of `sqlite-vec`, requiring manual cleanup logic in `clear_record_analysis`).

---

## Remediation Plan

### Phase 1: IPC & OCR Hardening (High Priority)
1. Fix `analyze_all_records` and `reprocess_all_records` to accept `force_ocr: bool`.
2. Update `ocr.rs` to guard against PDF processing in `image::open`.
3. Implement `force_ocr` logic in `indexer.rs`.

### Phase 2: Neural Engine Correction
1. Change `gelu_erf()` to `gelu()` in `gemma4.rs`.
2. Audit `Attention` forward pass for RoPE and Normalization alignment.

### Phase 3: UI & Telemetry Alignment
1. Standardize all `analysis-progress` event payloads to include `record_id`, `status`, `current`, and `total`.
2. Synchronize Svelte status strings with Rust constants.

### Phase 4: Persistence Cleanup
1. Remove redundant SQL calls in `AnalysisManager`.
2. Verify `clear_record_analysis` covers all virtual tables.
