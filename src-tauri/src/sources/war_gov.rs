use anyhow::{anyhow, Context, Result};
use csv::ReaderBuilder;
use rquest::{header, Client};
use serde_json::json;
use sha2::{Digest, Sha256};
use sqlx::{Row, SqlitePool};
use std::collections::{HashMap, HashSet};
use tokio::fs;
use uuid::Uuid;

use crate::library::LibraryManager;
use crate::models::{CsvRecord, SnapshotDiff, SyncReport};

pub const WAR_GOV_CSV_URL: &str = "https://www.war.gov/Portals/1/Interactive/2026/UFO/uap-csv.csv";

#[derive(Debug, Clone)]
struct ParsedOfficialRecord {
    csv: CsvRecord,
    stable_key: String,
    content_hash: String,
}

pub async fn sync_official_source(
    pool: &SqlitePool,
    library: &LibraryManager,
) -> Result<SyncReport> {
    let mut headers = header::HeaderMap::new();
    headers.insert(
        header::ACCEPT,
        header::HeaderValue::from_static(
            "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
        ),
    );
    headers.insert(
        header::ACCEPT_LANGUAGE,
        header::HeaderValue::from_static("en-US,en;q=0.9"),
    );
    headers.insert(
        header::REFERER,
        header::HeaderValue::from_static("https://www.war.gov/"),
    );

    let client = Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36")
        .default_headers(headers)
        .build()?;

    let bytes = client
        .get(WAR_GOV_CSV_URL)
        .send()
        .await
        .context("WAR.gov source request failed")?
        .error_for_status()
        .context("WAR.gov source returned an error status")?
        .bytes()
        .await
        .context("WAR.gov source body could not be read")?;

    sync_official_source_from_bytes(pool, library, &bytes).await
}

pub async fn sync_official_source_from_bytes(
    pool: &SqlitePool,
    library: &LibraryManager,
    bytes: &[u8],
) -> Result<SyncReport> {
    sync_official_source_from_bytes_inner(pool, library, bytes, WAR_GOV_CSV_URL).await
}

async fn sync_official_source_from_bytes_inner(
    pool: &SqlitePool,
    library: &LibraryManager,
    bytes: &[u8],
    upstream_url: &str,
) -> Result<SyncReport> {
    let content_hash = hash_bytes(bytes);
    let fetched_at = now();
    let snapshot_id = Uuid::new_v4().to_string();
    let snapshot_dir = library.snapshots_dir().join("war-gov");
    fs::create_dir_all(&snapshot_dir).await?;
    let snapshot_file = snapshot_dir.join(format!("{snapshot_id}.csv"));
    fs::write(&snapshot_file, bytes).await?;
    let snapshot_path = snapshot_file.to_string_lossy().into_owned();
    let records = parse_csv_records(bytes)?;
    let previous = previous_snapshot_records(pool).await?;

    sqlx::query(
        r#"
        INSERT INTO source_snapshots (
            id, source_name, upstream_url, release_label, fetched_at,
            content_hash, snapshot_path, record_count, status
        )
        VALUES (?, 'war.gov/UFO', ?, ?, ?, ?, ?, ?, 'completed')
        "#,
    )
    .bind(&snapshot_id)
    .bind(upstream_url)
    .bind(format!("WAR.gov UFO sync {}", &fetched_at[..10]))
    .bind(&fetched_at)
    .bind(&content_hash)
    .bind(&snapshot_path)
    .bind(i64::try_from(records.len()).unwrap_or(0))
    .execute(pool)
    .await?;

    let mut current_keys = HashSet::new();
    let mut diffs = Vec::new();
    let mut added = 0_usize;
    let mut changed = 0_usize;

    for record in &records {
        current_keys.insert(record.stable_key.clone());
        let record_json = serde_json::to_string(&record.csv)?;

        sqlx::query(
            r#"
            INSERT INTO source_snapshot_records (
                snapshot_id, stable_key, content_hash, title, document_url, record_json
            )
            VALUES (?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&snapshot_id)
        .bind(&record.stable_key)
        .bind(&record.content_hash)
        .bind(&record.csv.title)
        .bind(&record.csv.document_url)
        .bind(record_json)
        .execute(pool)
        .await?;

        let change_type = match previous.get(&record.stable_key) {
            None => {
                added += 1;
                Some("added")
            }
            Some(previous_hash) if previous_hash != &record.content_hash => {
                changed += 1;
                Some("changed")
            }
            _ => None,
        };

        if let Some(change_type) = change_type {
            insert_diff(
                pool,
                &snapshot_id,
                &record.stable_key,
                change_type,
                &record.csv.title,
                record.csv.document_url.as_deref(),
                previous.get(&record.stable_key).map(String::as_str),
                Some(&record.content_hash),
            )
            .await?;
            diffs.push(SnapshotDiff {
                change_type: change_type.to_string(),
                title: record.csv.title.clone(),
                document_url: record.csv.document_url.clone(),
                stable_key: record.stable_key.clone(),
            });
        }

        upsert_record(pool, &snapshot_id, record).await?;
    }

    let mut removed = 0_usize;
    for (stable_key, previous_hash) in previous {
        if !current_keys.contains(&stable_key) {
            removed += 1;
            let title = prior_title(pool, &stable_key)
                .await?
                .unwrap_or_else(|| stable_key.clone());
            insert_diff(
                pool,
                &snapshot_id,
                &stable_key,
                "removed",
                &title,
                None,
                Some(&previous_hash),
                None,
            )
            .await?;
            sqlx::query(
                "UPDATE records SET removed_from_source_at = ?, updated_at = CURRENT_TIMESTAMP WHERE stable_key = ? AND source_type = 'official'",
            )
            .bind(&fetched_at)
            .bind(&stable_key)
            .execute(pool)
            .await?;
            diffs.push(SnapshotDiff {
                change_type: "removed".to_string(),
                title,
                document_url: None,
                stable_key,
            });
        }
    }

    Ok(SyncReport {
        snapshot_id,
        upstream_url: upstream_url.to_string(),
        fetched_at,
        content_hash,
        snapshot_path,
        record_count: records.len(),
        added,
        changed,
        removed,
        diffs,
    })
}

#[cfg(test)]
fn parse_csv(bytes: &[u8]) -> Result<Vec<(String, CsvRecord)>> {
    parse_csv_records(bytes).map(|records| {
        records
            .into_iter()
            .map(|record| (record.stable_key, record.csv))
            .collect()
    })
}

fn parse_csv_records(bytes: &[u8]) -> Result<Vec<ParsedOfficialRecord>> {
    let mut reader = ReaderBuilder::new()
        .flexible(true)
        .trim(csv::Trim::All)
        .has_headers(true)
        .from_reader(bytes);

    let mut records = Vec::new();
    for result in reader.deserialize() {
        let csv: CsvRecord = result?;
        if csv.title.trim().is_empty() {
            continue;
        }
        let stable_key = stable_key(&csv);
        let content_hash = hash_json(&csv)?;
        records.push(ParsedOfficialRecord {
            csv,
            stable_key,
            content_hash,
        });
    }

    if records.is_empty() {
        return Err(anyhow!(
            "WAR.gov CSV parsed successfully but contained no usable records"
        ));
    }

    Ok(records)
}

async fn upsert_record(
    pool: &SqlitePool,
    snapshot_id: &str,
    record: &ParsedOfficialRecord,
) -> Result<()> {
    let id = existing_record_id(pool, &record.stable_key).await?;
    let record_id = id.unwrap_or_else(|| Uuid::new_v4().to_string());
    sqlx::query(
        r#"
        INSERT INTO records (
            id, title, agency, release_date, incident_date, incident_location,
            document_url, file_type, source_type, summary, stable_key,
            source_snapshot_id, content_hash, removed_from_source_at
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'official', ?, ?, ?, ?, NULL)
        ON CONFLICT(stable_key, source_type) DO UPDATE SET
            title = excluded.title,
            agency = excluded.agency,
            release_date = excluded.release_date,
            incident_date = excluded.incident_date,
            incident_location = excluded.incident_location,
            document_url = excluded.document_url,
            file_type = excluded.file_type,
            summary = excluded.summary,
            source_snapshot_id = excluded.source_snapshot_id,
            content_hash = excluded.content_hash,
            removed_from_source_at = NULL,
            updated_at = CURRENT_TIMESTAMP
        "#,
    )
    .bind(record_id)
    .bind(&record.csv.title)
    .bind(&record.csv.agency)
    .bind(&record.csv.release_date)
    .bind(&record.csv.incident_date)
    .bind(&record.csv.incident_location)
    .bind(&record.csv.document_url)
    .bind(&record.csv.doc_type)
    .bind(&record.csv.description)
    .bind(&record.stable_key)
    .bind(snapshot_id)
    .bind(&record.content_hash)
    .execute(pool)
    .await?;
    Ok(())
}

async fn existing_record_id(pool: &SqlitePool, stable_key: &str) -> Result<Option<String>> {
    Ok(sqlx::query_scalar::<_, String>(
        "SELECT id FROM records WHERE stable_key = ? AND source_type = 'official'",
    )
    .bind(stable_key)
    .fetch_optional(pool)
    .await?)
}

async fn previous_snapshot_records(pool: &SqlitePool) -> Result<HashMap<String, String>> {
    let snapshot_id = sqlx::query_scalar::<_, String>(
        "SELECT id FROM source_snapshots WHERE source_name = 'war.gov/UFO' AND status = 'completed' ORDER BY fetched_at DESC LIMIT 1",
    )
    .fetch_optional(pool)
    .await?;

    let Some(snapshot_id) = snapshot_id else {
        return Ok(HashMap::new());
    };

    let rows = sqlx::query(
        "SELECT stable_key, content_hash FROM source_snapshot_records WHERE snapshot_id = ?",
    )
    .bind(snapshot_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| {
            (
                row.get::<String, _>("stable_key"),
                row.get::<String, _>("content_hash"),
            )
        })
        .collect())
}

async fn insert_diff(
    pool: &SqlitePool,
    snapshot_id: &str,
    stable_key: &str,
    change_type: &str,
    title: &str,
    document_url: Option<&str>,
    previous_hash: Option<&str>,
    current_hash: Option<&str>,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO source_diffs (
            id, snapshot_id, stable_key, change_type, title, document_url,
            previous_hash, current_hash, created_at
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(Uuid::new_v4().to_string())
    .bind(snapshot_id)
    .bind(stable_key)
    .bind(change_type)
    .bind(title)
    .bind(document_url)
    .bind(previous_hash)
    .bind(current_hash)
    .bind(now())
    .execute(pool)
    .await?;
    Ok(())
}

async fn prior_title(pool: &SqlitePool, stable_key: &str) -> Result<Option<String>> {
    Ok(sqlx::query_scalar::<_, String>(
        "SELECT title FROM records WHERE stable_key = ? AND source_type = 'official'",
    )
    .bind(stable_key)
    .fetch_optional(pool)
    .await?)
}

fn stable_key(record: &CsvRecord) -> String {
    if let Some(url) = record
        .document_url
        .as_deref()
        .filter(|url| !url.trim().is_empty())
    {
        return format!("url:{}", url.trim());
    }
    let raw = format!(
        "{}|{}|{}",
        record.title.trim(),
        record.release_date.as_deref().unwrap_or("").trim(),
        record.agency.as_deref().unwrap_or("").trim()
    );
    format!("meta:{}", hash_bytes(raw.as_bytes()))
}

fn hash_json(record: &CsvRecord) -> Result<String> {
    let canonical = json!({
        "title": record.title,
        "agency": record.agency,
        "release_date": record.release_date,
        "incident_date": record.incident_date,
        "incident_location": record.incident_location,
        "document_url": record.document_url,
        "file_type": record.doc_type,
        "summary": record.description,
        "redaction": record.redaction,
    });
    Ok(hash_bytes(serde_json::to_string(&canonical)?.as_bytes()))
}

fn hash_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}

#[cfg(test)]
mod tests {
    use super::parse_csv;

    #[test]
    fn parses_fixture_csv() {
        let csv = b"Release Date,Title,Type,Agency,Incident Date,Incident Location,PDF | Image Link,Description Blurb\n2026-01-01,Case A,PDF,AARO,2025-12-01,Nevada,https://example.test/a.pdf,Summary\n";
        let records = parse_csv(csv).expect("parse");
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].1.title, "Case A");
    }
}
