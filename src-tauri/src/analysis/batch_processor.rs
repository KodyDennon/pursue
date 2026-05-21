use crate::analysis::AnalysisManager;
use crate::common::to_error;
use sqlx::{Row, SqlitePool};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};

pub struct BatchProcessor;

impl BatchProcessor {
    pub async fn index_all_records(
        analysis: Arc<AnalysisManager>,
        db: SqlitePool,
        app_handle: AppHandle,
    ) -> Result<usize, String> {
        let records = sqlx::query("SELECT id FROM records WHERE (analysis_status IS NULL OR analysis_status NOT IN ('indexed', 'completed')) AND local_path IS NOT NULL")
            .fetch_all(&db)
            .await
            .map_err(to_error)?;

        let count = records.len();
        if count == 0 {
            return Ok(0);
        }

        if analysis.is_busy() {
            return Err("Analysis already in progress".to_string());
        }

        analysis.set_busy(true);
        let handle = app_handle.clone();
        let analysis_clone = analysis.clone();

        tauri::async_runtime::spawn(async move {
            for (idx, row) in records.into_iter().enumerate() {
                let id: String = row.get("id");
                let current_idx = idx + 1;

                let _ = handle.emit(
                    "analysis-progress",
                    serde_json::json!({
                        "current": current_idx,
                        "total": count,
                        "status": "extracting-foundation",
                        "record_id": id
                    }),
                );

                if let Err(e) = analysis_clone
                    .index_record(&handle, &id, current_idx, count)
                    .await
                {
                    let _ = handle.emit(
                        "analysis-progress",
                        serde_json::json!({
                            "status": "record-failed",
                            "record_id": id,
                            "current": current_idx,
                            "total": count,
                            "error": format!("Indexing failed: {}", e)
                        }),
                    );
                }
            }

            let _ = handle.emit(
                "analysis-progress",
                serde_json::json!({
                    "current": count,
                    "total": count,
                    "status": "completed",
                    "record_id": null
                }),
            );

            analysis_clone.set_busy(false);
        });

        Ok(count)
    }

    pub async fn analyze_all_records(
        analysis: Arc<AnalysisManager>,
        db: SqlitePool,
        app_handle: AppHandle,
    ) -> Result<usize, String> {
        // WARMUP PHASE: Notify UI that we are querying the database
        let _ = app_handle.emit(
            "analysis-progress",
            serde_json::json!({
                "status": "initializing-batch",
                "msg": "Calculating foundation targets..."
            }),
        );

        let total_local = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM records WHERE local_path IS NOT NULL",
        )
        .fetch_one(&db)
        .await
        .map_err(to_error)? as usize;

        let records = sqlx::query("SELECT id FROM records WHERE (analysis_status IS NULL OR analysis_status NOT IN ('indexed', 'completed')) AND local_path IS NOT NULL")
            .fetch_all(&db)
            .await
            .map_err(to_error)?;

        let total_records = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM records")
            .fetch_one(&db)
            .await
            .map_err(to_error)? as usize;

        if analysis.is_busy() {
            return Err("Analysis already in progress".to_string());
        }

        let count = records.len();
        let remote_count = total_records - total_local;

        let _ = app_handle.emit(
            "analysis-progress",
            serde_json::json!({
                "status": "batch-planning",
                "msg": format!("Queueing {} pending audits. ({} records skipped; remote source targets).", count, remote_count)
            }),
        );

        if count == 0 {
            return Ok(0);
        }

        analysis.set_busy(true);

        let handle = app_handle.clone();
        let analysis_clone = analysis.clone();

        // RECORD PARALLELISM: Process records concurrently.
        tauri::async_runtime::spawn(async move {
            use futures::stream::StreamExt;
            use std::sync::atomic::{AtomicUsize, Ordering};

            let processed_count = Arc::new(AtomicUsize::new(0));
            let total_count = count;

            futures::stream::iter(records)
                .map(|row| {
                    let id: String = row.get("id");
                    let handle = handle.clone();
                    let analysis = analysis_clone.clone();
                    let processed = processed_count.clone();

                    async move {
                        let current_idx = processed.fetch_add(1, Ordering::SeqCst) + 1;

                        // Emit foundation start
                        let _ = handle.emit(
                            "analysis-progress",
                            serde_json::json!({
                                "current": current_idx,
                                "total": total_count,
                                "status": "extracting-foundation",
                                "record_id": id
                            }),
                        );

                        // 1. Foundation Phase (OCR / Vectorization)
                        if let Err(e) = analysis
                            .index_record(&handle, &id, current_idx, total_count)
                            .await
                        {
                            let _ = handle.emit(
                                "analysis-progress",
                                serde_json::json!({
                                    "status": "record-failed",
                                    "record_id": id,
                                    "current": current_idx,
                                    "total": total_count,
                                    "error": format!("Foundation failed: {}", e)
                                }),
                            );
                        } else {
                            // Success for this record
                            let _ = handle.emit(
                                "analysis-progress",
                                serde_json::json!({
                                    "current": current_idx,
                                    "total": total_count,
                                    "status": "record-completed",
                                    "record_id": id
                                }),
                            );
                        }
                        Ok::<(), String>(())
                    }
                })
                .buffer_unordered(2) // Reduced concurrency to prevent resource exhaustion during heavy OCR
                .collect::<Vec<_>>()
                .await;

            let _ = handle.emit(
                "analysis-progress",
                serde_json::json!({
                    "current": total_count,
                    "total": total_count,
                    "status": "completed",
                    "record_id": null
                }),
            );

            analysis_clone.set_busy(false);
        });

        Ok(count)
    }

    pub async fn reprocess_all_records(
        analysis: Arc<AnalysisManager>,
        db: SqlitePool,
        app_handle: AppHandle,
    ) -> Result<usize, String> {
        // WARMUP PHASE
        let _ = app_handle.emit(
            "analysis-progress",
            serde_json::json!({
                "status": "initializing-batch",
                "msg": "Purging foundation cache..."
            }),
        );

        // Get all records that have local content
        let records = sqlx::query("SELECT id FROM records WHERE local_path IS NOT NULL")
            .fetch_all(&db)
            .await
            .map_err(to_error)?;

        if analysis.is_busy() {
            return Err("Analysis already in progress".to_string());
        }

        let count = records.len();
        if count == 0 {
            return Ok(0);
        }

        // ATOMIC BULK PURGE: Reset the entire archive in one transaction.
        analysis.clear_all_analysis().await.map_err(to_error)?;

        analysis.set_busy(true);
        let handle = app_handle.clone();
        let analysis_clone = analysis.clone();

        // RECORD PARALLELISM: Concurrent re-audit loop.
        tauri::async_runtime::spawn(async move {
            use futures::stream::StreamExt;
            use std::sync::atomic::{AtomicUsize, Ordering};

            let processed_count = Arc::new(AtomicUsize::new(0));
            let total_count = count;

            futures::stream::iter(records)
                .map(|row| {
                    let id: String = row.get("id");
                    let handle = handle.clone();
                    let analysis = analysis_clone.clone();
                    let processed = processed_count.clone();

                    async move {
                        let current_idx = processed.fetch_add(1, Ordering::SeqCst) + 1;

                        let _ = handle.emit(
                            "analysis-progress",
                            serde_json::json!({
                                "current": current_idx,
                                "total": total_count,
                                "status": "extracting-foundation",
                                "record_id": id
                            }),
                        );

                        // Default to Neural Vision extraction
                        if let Err(e) = analysis
                            .index_record(&handle, &id, current_idx, total_count)
                            .await
                        {
                            let _ = handle.emit(
                                "analysis-progress",
                                serde_json::json!({
                                    "status": "record-failed",
                                    "record_id": id,
                                    "current": current_idx,
                                    "total": total_count,
                                    "error": format!("Forced OCR failed: {}", e)
                                }),
                            );
                        } else {
                            let _ = handle.emit(
                                "analysis-progress",
                                serde_json::json!({
                                    "current": current_idx,
                                    "total": total_count,
                                    "status": "record-completed",
                                    "record_id": id
                                }),
                            );
                        }
                        Ok::<(), String>(())
                    }
                })
                .buffer_unordered(2) // Reduced concurrency to prevent resource exhaustion during heavy OCR
                .collect::<Vec<_>>()
                .await;

            let _ = handle.emit(
                "analysis-progress",
                serde_json::json!({
                    "current": total_count,
                    "total": total_count,
                    "status": "completed",
                    "record_id": null
                }),
            );

            analysis_clone.set_busy(false);
        });

        Ok(count)
    }

    pub async fn synthesize_all_records(
        analysis: Arc<AnalysisManager>,
        db: SqlitePool,
        app_handle: AppHandle,
    ) -> Result<usize, String> {
        let _ = app_handle.emit(
            "analysis-progress",
            serde_json::json!({
                "status": "loading-model",
                "msg": "Calculating synthesis targets..."
            }),
        );

        let records = sqlx::query(
            "SELECT id FROM records WHERE (analysis_status = 'indexed') AND local_path IS NOT NULL",
        )
        .fetch_all(&db)
        .await
        .map_err(to_error)?;

        if analysis.is_busy() {
            return Err("Analysis already in progress".to_string());
        }

        let count = records.len();
        if count == 0 {
            let _ = app_handle.emit(
                "analysis-progress",
                serde_json::json!({
                    "current": 0,
                    "total": 0,
                    "status": "completed",
                    "record_id": null
                }),
            );
            return Ok(0);
        }

        analysis.set_busy(true);
        let handle = app_handle.clone();
        let analysis_clone = analysis.clone();

        tauri::async_runtime::spawn(async move {
            for (idx, row) in records.into_iter().enumerate() {
                let id: String = row.get("id");
                let current_idx = idx + 1;

                let _ = handle.emit(
                    "analysis-progress",
                    serde_json::json!({
                        "current": current_idx,
                        "total": count,
                        "status": "synthesizing-start",
                        "record_id": id
                    }),
                );

                if let Err(e) = analysis_clone.synthesize_intelligence(&handle, &id).await {
                    let _ = handle.emit(
                        "analysis-progress",
                        serde_json::json!({
                            "status": "record-failed",
                            "record_id": id,
                            "current": current_idx,
                            "total": count,
                            "error": format!("Synthesis failed: {}", e)
                        }),
                    );
                }
            }

            let _ = handle.emit(
                "analysis-progress",
                serde_json::json!({
                    "current": count,
                    "total": count,
                    "status": "completed",
                    "record_id": null
                }),
            );

            analysis_clone.set_busy(false);
        });

        Ok(count)
    }
}
