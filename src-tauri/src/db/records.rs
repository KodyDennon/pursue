use crate::models::{Record, RecordFilter, RecordSummary};
use sqlx::SqlitePool;

pub async fn list(
    pool: &SqlitePool,
    filter: Option<RecordFilter>,
) -> sqlx::Result<Vec<RecordSummary>> {
    let filter = filter.unwrap_or(RecordFilter {
        source_type: None,
        agency: None,
        local_only: None,
        query: None,
    });

    let mut rows = sqlx::query_as::<_, RecordSummary>(
        r#"
        SELECT
            r.id,
            r.title,
            r.agency,
            r.release_date,
            r.incident_date,
            r.incident_location,
            r.document_url,
            r.local_path,
            r.file_type,
            r.source_type,
            r.summary,
            r.stable_key,
            r.content_hash,
            r.removed_from_source_at,
            a.sha256 AS artifact_sha256,
            a.byte_size AS artifact_size,
            COALESCE(ar.status, r.analysis_status) AS analysis_status,
            r.intelligence_json,
            r.redaction_score,
            r.analysis_error,
            COUNT(re.entity_id) AS entity_count,
            (SELECT local_path FROM record_assets WHERE record_id = r.id AND asset_type = 'image' LIMIT 1) AS thumbnail_path
        FROM records r
        LEFT JOIN artifacts a ON a.record_id = r.id
        LEFT JOIN analysis_results ar ON ar.record_id = r.id
        LEFT JOIN record_entities re ON re.record_id = r.id
        WHERE (?1 IS NULL OR r.source_type = ?1)
          AND (?2 IS NULL OR r.agency = ?2)
          AND (?3 = 0 OR r.local_path IS NOT NULL)
          AND (
            ?4 IS NULL OR
            lower(r.title) LIKE '%' || lower(?4) || '%' OR
            lower(COALESCE(r.summary, '')) LIKE '%' || lower(?4) || '%' OR
            lower(COALESCE(r.agency, '')) LIKE '%' || lower(?4) || '%' OR
            lower(COALESCE(r.incident_location, '')) LIKE '%' || lower(?4) || '%'
          )
        GROUP BY r.id
        ORDER BY COALESCE(r.release_date, r.created_at) DESC, r.title ASC
        "#,
    )
    .bind(filter.source_type)
    .bind(filter.agency)
    .bind(if filter.local_only.unwrap_or(false) {
        1
    } else {
        0
    })
    .bind(filter.query)
    .fetch_all(pool)
    .await?;

    for row in &mut rows {
        if row.artifact_size.is_none() && row.local_path.is_some() {
            row.artifact_size = Some(0);
        }
    }

    Ok(rows)
}

pub async fn find_by_id(pool: &SqlitePool, id: &str) -> sqlx::Result<Option<Record>> {
    sqlx::query_as::<_, Record>(
        r#"
        SELECT id, title, agency, release_date, incident_date, incident_location,
               document_url, local_path, file_type, source_type, summary
        FROM records
        WHERE id = ?
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn find_summary_by_id(
    pool: &SqlitePool,
    id: &str,
) -> sqlx::Result<Option<RecordSummary>> {
    let records = list(
        pool,
        Some(RecordFilter {
            source_type: None,
            agency: None,
            local_only: None,
            query: None,
        }),
    )
    .await?;
    Ok(records.into_iter().find(|record| record.id == id))
}
