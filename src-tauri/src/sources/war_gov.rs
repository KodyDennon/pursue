use anyhow::{anyhow, Context, Result};
use csv::ReaderBuilder;
use reqwest::{header, Client};
use serde_json::json;
use sha2::{Digest, Sha256};
use sqlx::{Row, SqlitePool};
use std::collections::{HashMap, HashSet};
use tokio::fs;
use uuid::Uuid;

use crate::library::LibraryManager;
use crate::models::{CsvRecord, SnapshotDiff, SyncReport};

const WAR_GOV_CSV_URL: &str =
    "https://www.war.gov/Portals/1/Interactive/2026/UFO/uap-release001.csv";

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
            "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8,application/signed-exchange;v=b3;q=0.7",
        ),
    );
    headers.insert(
        header::ACCEPT_LANGUAGE,
        header::HeaderValue::from_static("en-US,en;q=0.9"),
    );
    headers.insert(
        header::ACCEPT_ENCODING,
        header::HeaderValue::from_static("gzip, deflate, br, zstd"),
    );
    headers.insert(
        "Priority",
        header::HeaderValue::from_static("u=0, i"),
    );
    headers.insert(
        "Sec-Ch-Ua",
        header::HeaderValue::from_static("\"Chromium\";v=\"148\", \"Google Chrome\";v=\"148\", \"Not/A)Brand\";v=\"99\""),
    );
    headers.insert(
        "Sec-Ch-Ua-Mobile",
        header::HeaderValue::from_static("?0"),
    );
    headers.insert(
        "Sec-Ch-Ua-Platform",
        header::HeaderValue::from_static("\"macOS\""),
    );
    headers.insert(
        "Upgrade-Insecure-Requests",
        header::HeaderValue::from_static("1"),
    );

    let client = Client::builder()
        .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/148.0.0.0 Safari/537.36")
        .default_headers(headers)
        .cookie_store(true)
        .build()?;

    // Step 1: Prime session
    tauri_plugin_log::log::info!("Priming session at https://www.war.gov/UFO/...");
    let _ = client.get("https://www.war.gov/UFO/").send().await;

    // Step 2: Fetch CSV with proper referer and same-origin headers
    let response = client
        .get(WAR_GOV_CSV_URL)
        .header(header::REFERER, "https://www.war.gov/UFO/")
        .header("Sec-Fetch-Dest", "empty")
        .header("Sec-Fetch-Mode", "cors")
        .header("Sec-Fetch-Site", "same-origin")
        .header(header::ACCEPT, "*/*")
        .send()
        .await
        .context("WAR.gov source request failed")?;

    let status = response.status();
    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown")
        .to_string();
    
    if !status.is_success() {
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "Could not read body".to_string());
        tauri_plugin_log::log::error!("WAR.gov sync failed with status {}: {}", status, body);
        // ... (existing fallback logic)
        if let Ok(home) = std::env::var("HOME") {
            let local_path = std::path::PathBuf::from(home)
                .join("Downloads")
                .join("uap-csv.csv");
            if local_path.exists() {
                tauri_plugin_log::log::info!("Attempting fallback to local CSV: {:?}", local_path);
                if let Ok(local_bytes) = fs::read(&local_path).await {
                    return sync_official_source_from_bytes(pool, library, &local_bytes).await;
                }
            }
        }

        return Err(anyhow!(
            "WAR.gov source returned error status {}: {}",
            status,
            body
        ));
    }

    let bytes = response
        .bytes()
        .await
        .context("WAR.gov source body could not be read")?;

    tauri_plugin_log::log::info!(
        "Received CSV response: status={}, type={}, len={}, prefix={:?}",
        status,
        content_type,
        bytes.len(),
        String::from_utf8_lossy(&bytes[..bytes.len().min(100)])
    );

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
        let title = record.csv.title.as_deref().unwrap_or("Untitled");

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
        .bind(title)
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
            let title = record.csv.title.as_deref().unwrap_or("Untitled");
            insert_diff(
                pool,
                &snapshot_id,
                &record.stable_key,
                change_type,
                title,
                record.csv.document_url.as_deref(),
                previous.get(&record.stable_key).map(String::as_str),
                Some(&record.content_hash),
            )
            .await?;
            diffs.push(SnapshotDiff {
                change_type: change_type.to_string(),
                title: title.to_string(),
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

fn parse_csv_records(bytes: &[u8]) -> Result<Vec<ParsedOfficialRecord>> {
    if bytes.starts_with(b"<!DOCTYPE") || bytes.starts_with(b"<HTML") || bytes.starts_with(b"<html") {
        let sample = String::from_utf8_lossy(&bytes[..bytes.len().min(200)]);
        return Err(anyhow!("Received HTML instead of CSV. Content starts with: {}", sample));
    }

    let mut clean_bytes = bytes;
    if clean_bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        clean_bytes = &clean_bytes[3..];
    }
    let data = String::from_utf8_lossy(clean_bytes);

    let mut reader = ReaderBuilder::new()
        .flexible(true)
        .trim(csv::Trim::All)
        .has_headers(true)
        .from_reader(data.as_bytes());

    let headers = reader.headers()?.clone();
    tauri_plugin_log::log::info!("CSV Headers found: {:?}", headers);
    let header_map: HashMap<String, usize> = headers
        .iter()
        .enumerate()
        .map(|(i, name)| (name.trim().to_lowercase(), i))
        .collect();

    let mut records_map = HashMap::new();
    let mut total_malformed = 0;
    let mut first_error = None;

    for (_i, result) in reader.records().enumerate() {
        let record = match result {
            Ok(r) => r,
            Err(e) => {
                if first_error.is_none() { first_error = Some(e.to_string()); }
                total_malformed += 1;
                continue;
            }
        };

        let get_field = |name: &str| -> Option<String> {
            header_map.get(&name.to_lowercase()).and_then(|&idx| {
                record.get(idx).map(|val| val.trim().to_string())
            }).filter(|s| !s.is_empty())
        };

        let title = match get_field("Title") {
            Some(t) => t,
            None => continue,
        };

        let csv = CsvRecord {
            redaction: get_field("Redaction"),
            release_date: get_field("Release Date"),
            title: Some(title.clone()),
            doc_type: get_field("Type"),
            video_pairing: get_field("Video Pairing"),
            pdf_pairing: get_field("PDF Pairing"),
            description: get_field("Description Blurb"),
            dvids_video_id: get_field("DVIDS Video ID"),
            video_title: get_field("Video Title"),
            agency: get_field("Agency"),
            incident_date: get_field("Incident Date"),
            incident_location: get_field("Incident Location"),
            document_url: get_field("PDF | Image Link"),
            modal_image: get_field("Modal Image"),
            image_alt_text: get_field("Image Alt Text"),
            image_virin: get_field("Image VIRIN"),
        };

        let stable_key = stable_key(&csv);
        let content_hash = hash_json(&csv)?;

        records_map
            .entry(stable_key.clone())
            .or_insert(ParsedOfficialRecord {
                csv,
                stable_key,
                content_hash,
            });
    }

    let records: Vec<ParsedOfficialRecord> = records_map.into_values().collect();

    if records.is_empty() {
        let err_msg = if let Some(e) = first_error {
            format!("WAR.gov CSV contained no usable records. First error: {} ({} malformed rows skipped)", e, total_malformed)
        } else {
            format!("WAR.gov CSV contained no usable records ({} empty or malformed rows skipped)", total_malformed)
        };
        return Err(anyhow!(err_msg));
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
    
    let title = record.csv.title.as_deref().unwrap_or("Untitled").trim();
    let agency = record.csv.agency.as_deref().map(str::trim);
    let summary = record.csv.description.as_deref().map(str::trim);
    let incident_location = record.csv.incident_location.as_deref().map(str::trim);

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
    .bind(title)
    .bind(agency)
    .bind(&record.csv.release_date)
    .bind(&record.csv.incident_date)
    .bind(incident_location)
    .bind(&record.csv.document_url)
    .bind(&record.csv.doc_type)
    .bind(summary)
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

#[allow(clippy::too_many_arguments)]
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
    let title = record.title.as_deref().unwrap_or("").trim();
    let date = record.release_date.as_deref().unwrap_or("").trim();
    let agency = record.agency.as_deref().unwrap_or("").trim();

    let url = record.document_url.as_deref().unwrap_or("").trim();
    let has_real_url = url.starts_with("http://") || url.starts_with("https://");

    // Normalize title for key generation (remove leading zeros from numbers)
    // e.g. "Cable 001" -> "Cable 1"
    let normalized_title = normalize_title(title);

    if has_real_url {
        format!("url:{}|title:{}", url, normalized_title)
    } else {
        let raw = format!("{}|{}|{}|{}", normalized_title, date, agency, url);
        format!("meta:{}", hash_bytes(raw.as_bytes()))
    }
}

fn normalize_title(title: &str) -> String {
    let mut normalized = String::new();
    let mut current_num = String::new();

    for c in title.chars() {
        if c.is_ascii_digit() {
            current_num.push(c);
        } else {
            if !current_num.is_empty() {
                // Remove leading zeros from the number
                let parsed = current_num.parse::<u64>().unwrap_or(0);
                normalized.push_str(&parsed.to_string());
                current_num.clear();
            }
            normalized.push(c);
        }
    }
    if !current_num.is_empty() {
        let parsed = current_num.parse::<u64>().unwrap_or(0);
        normalized.push_str(&parsed.to_string());
    }
    normalized.to_lowercase()
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
        "video_pairing": record.video_pairing,
        "pdf_pairing": record.pdf_pairing,
        "dvids_video_id": record.dvids_video_id,
        "video_title": record.video_title,
        "modal_image": record.modal_image,
        "image_alt_text": record.image_alt_text,
        "image_virin": record.image_virin,
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
