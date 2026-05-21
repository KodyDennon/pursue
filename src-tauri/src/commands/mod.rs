use crate::library::LibraryManager;
use crate::models::DatabaseStatus;
use anyhow::Result;
use sqlx::{Row, SqlitePool};

pub mod analysis;
pub mod cases;
pub mod records;
pub mod system;

pub use analysis::*;
pub use cases::*;
pub use records::*;
pub use system::*;

pub use crate::AppState;

pub use crate::common::{now, to_error};

pub async fn count_scalar(db: &SqlitePool, sql: &str) -> Result<i64> {
    Ok(sqlx::query_scalar::<_, i64>(sql).fetch_one(db).await?)
}

pub async fn database_status(db: &SqlitePool, library: &LibraryManager) -> Result<DatabaseStatus> {
    let latest_snapshot = sqlx::query(
        r#"
        SELECT fetched_at, upstream_url, record_count
        FROM source_snapshots
        WHERE status = 'completed'
        ORDER BY fetched_at DESC
        LIMIT 1
        "#,
    )
    .fetch_optional(db)
    .await?;

    let counts = sqlx::query(
        r#"
        SELECT 
            (SELECT COUNT(*) FROM records) as total_records,
            (SELECT COUNT(*) FROM records WHERE source_type = 'official') as official_records,
            (SELECT COUNT(*) FROM records WHERE source_type = 'manual') as manual_records,
            (SELECT COUNT(*) FROM records WHERE local_path IS NOT NULL) as local_records,
            (SELECT COALESCE(SUM(byte_size), 0) FROM artifacts) as artifact_bytes,
            (SELECT COUNT(*) FROM records WHERE analysis_status IN ('completed', 'indexed')) as analyzed_records,
            (SELECT COUNT(*) FROM records WHERE analysis_status = 'failed') as failed_analysis_records,
            (SELECT COUNT(*) FROM records WHERE analysis_status IS NULL OR analysis_status NOT IN ('completed', 'indexed')) as unanalyzed_count,
            (SELECT COUNT(*) FROM records WHERE analysis_status = 'completed') as completed_count,
            (SELECT COUNT(*) FROM records WHERE analysis_status IS NULL OR analysis_status = 'pending') as pending_count,
            (SELECT COUNT(*) FROM artifacts) as artifact_count,
            (SELECT COUNT(*) FROM analysis_chunks) as analysis_chunks,
            (SELECT COUNT(*) FROM vec_analysis_chunks) as vector_chunks,
            (SELECT COUNT(*) FROM entities) as entity_count,
            (SELECT COUNT(*) FROM cases) as case_count,
            (SELECT COUNT(*) FROM source_snapshots) as source_snapshots,
            (SELECT COUNT(*) FROM records WHERE source_type = 'official' AND document_url IS NOT NULL AND document_url != '') as downloadable_records
        "#
    )
    .fetch_one(db)
    .await?;

    let db_path = library.app_data_dir().join("pursue.db");
    let database_bytes = std::fs::metadata(&db_path)
        .map(|m| m.len() as i64)
        .unwrap_or(0);

    Ok(DatabaseStatus {
        app_data_dir: library.app_data_dir().to_string_lossy().into_owned(),
        database_path: db_path.to_string_lossy().into_owned(),
        database_bytes,
        library_path: library.library_dir().to_string_lossy().into_owned(),
        snapshots_path: library.snapshots_dir().to_string_lossy().into_owned(),
        exports_path: library.exports_dir().to_string_lossy().into_owned(),
        total_records: counts.get("total_records"),
        official_records: counts.get("official_records"),
        manual_records: counts.get("manual_records"),
        downloadable_records: counts.get("downloadable_records"),
        local_records: counts.get("local_records"),
        artifact_count: counts.get("artifact_count"),
        artifact_bytes: counts.get("artifact_bytes"),
        analyzed_records: counts.get("analyzed_records"),
        failed_analysis_records: counts.get("failed_analysis_records"),
        analysis_chunks: counts.get("analysis_chunks"),
        vector_chunks: counts.get("vector_chunks"),
        entity_count: counts.get("entity_count"),
        case_count: counts.get("case_count"),
        source_snapshots: counts.get("source_snapshots"),
        latest_snapshot_at: latest_snapshot
            .as_ref()
            .map(|row| row.get::<String, _>("fetched_at")),
        latest_snapshot_url: latest_snapshot
            .as_ref()
            .map(|row| row.get::<String, _>("upstream_url")),
        latest_snapshot_records: latest_snapshot
            .as_ref()
            .map(|row| row.get::<i64, _>("record_count")),
        active_download_jobs: count_scalar(
            db,
            "SELECT COUNT(*) FROM download_jobs WHERE status IN ('queued', 'running')",
        )
        .await?,
        total_count: counts.get("total_records"),
        total_size: counts.get("artifact_bytes"),
        unanalyzed_count: counts.get("unanalyzed_count"),
        completed_count: counts.get("completed_count"),
        pending_count: counts.get("pending_count"),
    })
}
