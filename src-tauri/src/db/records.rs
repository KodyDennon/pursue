use crate::models::{Record, RecordFilter, RecordPage, RecordSummary};
use sqlx::SqlitePool;

/// Builds a safe FTS5 MATCH expression from free-text user input. Every token is individually
/// quoted (with embedded quotes escaped by doubling, FTS5's own escaping convention) and given a
/// trailing `*` for prefix matching, then space-joined — FTS5 implicitly ANDs space-separated
/// phrase groups, so this matches rows where every token appears as a prefix somewhere in the
/// indexed columns, order-independent (matching the old LIKE-based "contains all these words"
/// intent) without ever passing raw user input to MATCH, which would otherwise let FTS5 query
/// syntax (`-`, `"`, `AND`, `NEAR`, ...) in user search text throw a query error.
pub fn to_fts5_query(raw: &str) -> Option<String> {
    let tokens: Vec<String> = raw
        .split_whitespace()
        .map(|token| format!("\"{}\"*", token.replace('"', "\"\"")))
        .collect();
    if tokens.is_empty() {
        None
    } else {
        Some(tokens.join(" "))
    }
}

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
            r.release_label,
            r.source_asset_class,
            r.dvids_video_id,
            r.video_title,
            r.video_pairing,
            r.pdf_pairing,
            r.modal_image,
            r.image_alt_text,
            r.image_virin,
            r.local_path,
            r.file_type,
            r.source_type,
            r.summary,
            r.stable_key,
            r.content_hash,
            r.removed_from_source_at,
            a.sha256 AS artifact_sha256,
            COALESCE(a.byte_size, 0) AS artifact_size,
            r.analysis_status,
            r.intelligence_json,
            r.redaction_score,
            r.analysis_error,
            (SELECT COUNT(*) FROM record_entities WHERE record_id = r.id) AS entity_count,
            COALESCE(r.thumbnail_path, (SELECT local_path FROM record_assets WHERE record_id = r.id AND asset_type = 'image' LIMIT 1)) AS thumbnail_path
        FROM records r
        LEFT JOIN artifacts a ON a.relative_path = r.local_path
        WHERE (?1 IS NULL OR r.source_type = ?1)
          AND (?2 IS NULL OR r.agency = ?2)
          AND (?3 = 0 OR r.local_path IS NOT NULL)
          AND (
            ?4 IS NULL OR
            r.rowid IN (SELECT rowid FROM records_fts WHERE records_fts MATCH ?4)
          )
        GROUP BY r.id
        ORDER BY r.created_at DESC, r.title ASC
        "#,
    )
    .bind(filter.source_type)
    .bind(filter.agency)
    .bind(if filter.local_only.unwrap_or(false) {
        1
    } else {
        0
    })
    .bind(filter.query.as_deref().and_then(to_fts5_query))
    .fetch_all(pool)
    .await?;

    for row in &mut rows {
        if row.artifact_size.is_none() && row.local_path.is_some() {
            row.artifact_size = Some(0);
        }
    }

    Ok(rows)
}

pub async fn list_page(
    pool: &SqlitePool,
    filter: Option<RecordFilter>,
    limit: i64,
    offset: i64,
) -> sqlx::Result<RecordPage> {
    let filter = filter.unwrap_or(RecordFilter {
        source_type: None,
        agency: None,
        local_only: None,
        query: None,
    });
    let limit = limit.clamp(25, 500);
    let offset = offset.max(0);
    let local_only = if filter.local_only.unwrap_or(false) {
        1
    } else {
        0
    };
    let fts_query = filter.query.as_deref().and_then(to_fts5_query);

    let total: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM records r
        WHERE (?1 IS NULL OR r.source_type = ?1)
          AND (?2 IS NULL OR r.agency = ?2)
          AND (?3 = 0 OR r.local_path IS NOT NULL)
          AND (
            ?4 IS NULL OR
            r.rowid IN (SELECT rowid FROM records_fts WHERE records_fts MATCH ?4)
          )
        "#,
    )
    .bind(filter.source_type.clone())
    .bind(filter.agency.clone())
    .bind(local_only)
    .bind(fts_query.clone())
    .fetch_one(pool)
    .await?;

    let mut records = sqlx::query_as::<_, RecordSummary>(
        r#"
        SELECT
            r.id,
            r.title,
            r.agency,
            r.release_date,
            r.incident_date,
            r.incident_location,
            r.document_url,
            r.release_label,
            r.source_asset_class,
            r.dvids_video_id,
            r.video_title,
            r.video_pairing,
            r.pdf_pairing,
            r.modal_image,
            r.image_alt_text,
            r.image_virin,
            r.local_path,
            r.file_type,
            r.source_type,
            r.summary,
            r.stable_key,
            r.content_hash,
            r.removed_from_source_at,
            a.sha256 AS artifact_sha256,
            COALESCE(a.byte_size, 0) AS artifact_size,
            r.analysis_status,
            NULL AS intelligence_json,
            r.redaction_score,
            r.analysis_error,
            0 AS entity_count,
            r.thumbnail_path AS thumbnail_path
        FROM records r
        LEFT JOIN artifacts a ON a.relative_path = r.local_path
        WHERE (?1 IS NULL OR r.source_type = ?1)
          AND (?2 IS NULL OR r.agency = ?2)
          AND (?3 = 0 OR r.local_path IS NOT NULL)
          AND (
            ?4 IS NULL OR
            r.rowid IN (SELECT rowid FROM records_fts WHERE records_fts MATCH ?4)
          )
        GROUP BY r.id
        ORDER BY r.created_at DESC, r.title ASC
        LIMIT ?5 OFFSET ?6
        "#,
    )
    .bind(filter.source_type)
    .bind(filter.agency)
    .bind(local_only)
    .bind(fts_query)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;

    for row in &mut records {
        if row.artifact_size.is_none() && row.local_path.is_some() {
            row.artifact_size = Some(0);
        }
    }

    Ok(RecordPage {
        records,
        total,
        limit,
        offset,
    })
}

pub async fn find_by_id(pool: &SqlitePool, id: &str) -> sqlx::Result<Option<Record>> {
    sqlx::query_as::<_, Record>(
        r#"
        SELECT id, title, agency, release_date, incident_date, incident_location,
               document_url, release_label, source_asset_class, dvids_video_id,
               video_title, video_pairing, pdf_pairing, modal_image, image_alt_text,
               image_virin, local_path, file_type, source_type, summary
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
    sqlx::query_as::<_, RecordSummary>(
        r#"
        SELECT
            r.id,
            r.title,
            r.agency,
            r.release_date,
            r.incident_date,
            r.incident_location,
            r.document_url,
            r.release_label,
            r.source_asset_class,
            r.dvids_video_id,
            r.video_title,
            r.video_pairing,
            r.pdf_pairing,
            r.modal_image,
            r.image_alt_text,
            r.image_virin,
            r.local_path,
            r.file_type,
            r.source_type,
            r.summary,
            r.stable_key,
            r.content_hash,
            r.removed_from_source_at,
            a.sha256 AS artifact_sha256,
            COALESCE(a.byte_size, 0) AS artifact_size,
            r.analysis_status,
            r.intelligence_json,
            r.redaction_score,
            r.analysis_error,
            (SELECT COUNT(*) FROM record_entities WHERE record_id = r.id) AS entity_count,
            COALESCE(r.thumbnail_path, (SELECT local_path FROM record_assets WHERE record_id = r.id AND asset_type = 'image' LIMIT 1)) AS thumbnail_path
        FROM records r
        LEFT JOIN artifacts a ON a.relative_path = r.local_path
        WHERE r.id = ?
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_fts5_query_quotes_and_escapes_every_token() {
        assert_eq!(to_fts5_query("AARO sensor"), Some("\"AARO\"* \"sensor\"*".to_string()));
        assert_eq!(
            to_fts5_query("weird\"quote"),
            Some("\"weird\"\"quote\"*".to_string())
        );
        // FTS5 operators/syntax in raw user input must never reach MATCH unescaped.
        assert_eq!(
            to_fts5_query("foo OR bar -baz"),
            Some("\"foo\"* \"OR\"* \"bar\"* \"-baz\"*".to_string())
        );
        assert_eq!(to_fts5_query("   "), None);
        assert_eq!(to_fts5_query(""), None);
    }

    async fn seed_record(pool: &SqlitePool, id: &str, title: &str, agency: &str) {
        sqlx::query(
            "INSERT INTO records (id, title, agency, source_type) VALUES (?, ?, ?, 'official')",
        )
        .bind(id)
        .bind(title)
        .bind(agency)
        .execute(pool)
        .await
        .expect("seed record");
    }

    #[tokio::test]
    async fn list_page_query_filter_uses_the_fts_index_via_triggers() {
        let pool = crate::db::test_pool().await.expect("test pool");
        seed_record(&pool, "rec-1", "UAP Sensor Anomaly Report", "Department of War").await;
        seed_record(&pool, "rec-2", "Routine Budget Memo", "Department of War").await;

        let filter = RecordFilter {
            source_type: None,
            agency: None,
            local_only: None,
            query: Some("sensor".to_string()),
        };
        let page = list_page(&pool, Some(filter), 25, 0).await.expect("list_page");

        assert_eq!(page.total, 1);
        assert_eq!(page.records.len(), 1);
        assert_eq!(page.records[0].id, "rec-1");
    }

    #[tokio::test]
    async fn list_page_query_filter_reflects_updates_via_the_update_trigger() {
        let pool = crate::db::test_pool().await.expect("test pool");
        seed_record(&pool, "rec-3", "Original Title", "NASA").await;

        sqlx::query("UPDATE records SET title = ? WHERE id = ?")
            .bind("Renamed Anomaly Dossier")
            .bind("rec-3")
            .execute(&pool)
            .await
            .expect("update title");

        let stale_filter = RecordFilter {
            source_type: None,
            agency: None,
            local_only: None,
            query: Some("Original".to_string()),
        };
        let stale_page = list_page(&pool, Some(stale_filter), 25, 0)
            .await
            .expect("list_page");
        assert_eq!(stale_page.total, 0, "old title must no longer match");

        let fresh_filter = RecordFilter {
            source_type: None,
            agency: None,
            local_only: None,
            query: Some("Renamed".to_string()),
        };
        let fresh_page = list_page(&pool, Some(fresh_filter), 25, 0)
            .await
            .expect("list_page");
        assert_eq!(fresh_page.total, 1);
        assert_eq!(fresh_page.records[0].id, "rec-3");
    }

    #[tokio::test]
    async fn list_page_query_filter_excludes_deleted_records_via_the_delete_trigger() {
        let pool = crate::db::test_pool().await.expect("test pool");
        seed_record(&pool, "rec-4", "Deletable Sighting Report", "FBI").await;

        sqlx::query("DELETE FROM records WHERE id = ?")
            .bind("rec-4")
            .execute(&pool)
            .await
            .expect("delete record");

        let filter = RecordFilter {
            source_type: None,
            agency: None,
            local_only: None,
            query: Some("Deletable".to_string()),
        };
        let page = list_page(&pool, Some(filter), 25, 0).await.expect("list_page");
        assert_eq!(page.total, 0);
    }

    #[tokio::test]
    async fn list_page_with_no_query_returns_everything() {
        let pool = crate::db::test_pool().await.expect("test pool");
        seed_record(&pool, "rec-5", "First Record", "CIA").await;
        seed_record(&pool, "rec-6", "Second Record", "CIA").await;

        let page = list_page(&pool, None, 25, 0).await.expect("list_page");
        assert_eq!(page.total, 2);
    }
}
