use anyhow::{anyhow, Result};
use csv::ReaderBuilder;
use percent_encoding::percent_decode_str;
use serde_json::json;
use sha2::{Digest, Sha256};
use sqlx::{Row, SqlitePool};
use std::collections::{HashMap, HashSet};
use tokio::fs;
use url::Url;
use uuid::Uuid;

use crate::library::LibraryManager;
use crate::models::{CsvRecord, SnapshotDiff, SyncReport};

pub const WAR_GOV_CSV_URL: &str =
    "https://www.war.gov/Portals/1/Interactive/2026/UFO/uap-data.csv";

#[derive(Debug, Clone)]
struct ParsedOfficialRecord {
    csv: CsvRecord,
    stable_key: String,
    content_hash: String,
    release_label: Option<String>,
}

#[derive(Debug)]
struct RepairCandidate {
    id: String,
    stable_key: Option<String>,
    local_path: Option<String>,
    removed_from_source_at: Option<String>,
    updated_at: Option<String>,
}

pub async fn sync_official_source_from_bytes_with_url(
    pool: &SqlitePool,
    library: &LibraryManager,
    bytes: &[u8],
    upstream_url: &str,
) -> Result<SyncReport> {
    sync_official_source_from_bytes_inner(pool, library, bytes, upstream_url).await
}

async fn sync_official_source_from_bytes_inner(
    pool: &SqlitePool,
    library: &LibraryManager,
    bytes: &[u8],
    upstream_url: &str,
) -> Result<SyncReport> {
    let _ = repair_official_record_identities(pool).await?;

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
    if bytes.starts_with(b"<!DOCTYPE") || bytes.starts_with(b"<HTML") || bytes.starts_with(b"<html")
    {
        let sample = String::from_utf8_lossy(&bytes[..bytes.len().min(200)]);
        return Err(anyhow!(
            "Received HTML instead of CSV. Content starts with: {}",
            sample
        ));
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

    for result in reader.records() {
        let record = match result {
            Ok(r) => r,
            Err(e) => {
                if first_error.is_none() {
                    first_error = Some(e.to_string());
                }
                total_malformed += 1;
                continue;
            }
        };

        let get_field = |name: &str| -> Option<String> {
            header_map
                .get(&name.to_lowercase())
                .and_then(|&idx| record.get(idx).map(|val| val.trim().to_string()))
                .filter(|s| !s.is_empty())
        };

        let title = match get_field("Title") {
            Some(t) => t,
            None => continue,
        };

        let document_url = get_field("PDF | Image Link").map(|url| normalize_source_url(&url));

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
            document_url,
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
                release_label: None,
            });
    }

    let mut records: Vec<ParsedOfficialRecord> = records_map.into_values().collect();
    let release_labels = release_labels_for(&records);
    for record in &mut records {
        record.release_label = record
            .csv
            .release_date
            .as_ref()
            .and_then(|date| release_labels.get(date))
            .cloned();
    }

    if records.is_empty() {
        let err_msg = if let Some(e) = first_error {
            format!("WAR.gov CSV contained no usable records. First error: {} ({} malformed rows skipped)", e, total_malformed)
        } else {
            format!(
                "WAR.gov CSV contained no usable records ({} empty or malformed rows skipped)",
                total_malformed
            )
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
    let release_label = record.release_label.as_deref();
    let source_asset_class = source_asset_class(&record.csv);

    sqlx::query(
        r#"
        INSERT INTO records (
            id, title, agency, release_date, incident_date, incident_location,
            document_url, release_label, source_asset_class, dvids_video_id,
            video_title, video_pairing, pdf_pairing, modal_image, image_alt_text,
            image_virin, file_type, source_type, summary, stable_key,
            source_snapshot_id, content_hash, removed_from_source_at
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'official', ?, ?, ?, ?, NULL)
        ON CONFLICT(stable_key, source_type) DO UPDATE SET
            title = excluded.title,
            agency = excluded.agency,
            release_date = excluded.release_date,
            incident_date = excluded.incident_date,
            incident_location = excluded.incident_location,
            document_url = excluded.document_url,
            release_label = excluded.release_label,
            source_asset_class = excluded.source_asset_class,
            dvids_video_id = excluded.dvids_video_id,
            video_title = excluded.video_title,
            video_pairing = excluded.video_pairing,
            pdf_pairing = excluded.pdf_pairing,
            modal_image = excluded.modal_image,
            image_alt_text = excluded.image_alt_text,
            image_virin = excluded.image_virin,
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
    .bind(release_label)
    .bind(&source_asset_class)
    .bind(&record.csv.dvids_video_id)
    .bind(&record.csv.video_title)
    .bind(&record.csv.video_pairing)
    .bind(&record.csv.pdf_pairing)
    .bind(&record.csv.modal_image)
    .bind(&record.csv.image_alt_text)
    .bind(&record.csv.image_virin)
    .bind(&record.csv.doc_type)
    .bind(summary)
    .bind(&record.stable_key)
    .bind(snapshot_id)
    .bind(&record.content_hash)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn repair_official_record_identities(pool: &SqlitePool) -> Result<usize> {
    let rows = sqlx::query(
        r#"
        SELECT id, stable_key, title, agency, release_date, document_url, local_path,
               removed_from_source_at, updated_at
        FROM records
        WHERE source_type = 'official'
        "#,
    )
    .fetch_all(pool)
    .await?;

    let mut groups: HashMap<String, Vec<RepairCandidate>> = HashMap::new();
    for row in rows {
        let csv = CsvRecord {
            redaction: None,
            release_date: row.get("release_date"),
            title: row.get("title"),
            doc_type: None,
            video_pairing: None,
            pdf_pairing: None,
            description: None,
            dvids_video_id: None,
            video_title: None,
            agency: row.get("agency"),
            incident_date: None,
            incident_location: None,
            document_url: row.get("document_url"),
            modal_image: None,
            image_alt_text: None,
            image_virin: None,
        };
        let canonical_key = stable_key(&csv);
        groups
            .entry(canonical_key.clone())
            .or_default()
            .push(RepairCandidate {
                id: row.get("id"),
                stable_key: row.get("stable_key"),
                local_path: row.get("local_path"),
                removed_from_source_at: row.get("removed_from_source_at"),
                updated_at: row.get("updated_at"),
            });
    }

    let mut repaired = 0_usize;

    for (canonical_key, mut candidates) in groups {
        candidates.sort_by(|left, right| {
            candidate_rank(right, &canonical_key).cmp(&candidate_rank(left, &canonical_key))
        });

        let keeper = candidates[0].id.clone();
        if candidates.len() == 1 {
            if candidates[0].stable_key.as_deref() != Some(canonical_key.as_str()) {
                sqlx::query(
                    "UPDATE records SET stable_key = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
                )
                .bind(&canonical_key)
                .bind(&keeper)
                .execute(pool)
                .await?;
                repaired += 1;
            }
            continue;
        }

        for duplicate in candidates.iter().skip(1) {
            merge_duplicate_record(pool, &keeper, &duplicate.id).await?;
            repaired += 1;
        }

        sqlx::query(
            "UPDATE records SET stable_key = ?, removed_from_source_at = NULL, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
        )
        .bind(&canonical_key)
        .bind(&keeper)
        .execute(pool)
        .await?;
    }

    Ok(repaired)
}

fn candidate_rank(candidate: &RepairCandidate, canonical_key: &str) -> (u8, u8, u8, String) {
    (
        u8::from(candidate.removed_from_source_at.is_none()),
        u8::from(candidate.local_path.is_some()),
        u8::from(candidate.stable_key.as_deref() == Some(canonical_key)),
        candidate.updated_at.clone().unwrap_or_default(),
    )
}

async fn merge_duplicate_record(pool: &SqlitePool, keeper: &str, duplicate: &str) -> Result<()> {
    sqlx::query(
        r#"
        UPDATE records
        SET local_path = COALESCE(local_path, (SELECT local_path FROM records WHERE id = ?)),
            thumbnail_path = COALESCE(thumbnail_path, (SELECT thumbnail_path FROM records WHERE id = ?)),
            intelligence_json = COALESCE(intelligence_json, (SELECT intelligence_json FROM records WHERE id = ?)),
            redaction_score = COALESCE(redaction_score, (SELECT redaction_score FROM records WHERE id = ?)),
            analysis_error = COALESCE(analysis_error, (SELECT analysis_error FROM records WHERE id = ?)),
            analysis_status = CASE
                WHEN analysis_status IN ('completed', 'indexed') THEN analysis_status
                ELSE COALESCE((SELECT analysis_status FROM records WHERE id = ?), analysis_status)
            END,
            removed_from_source_at = CASE
                WHEN removed_from_source_at IS NULL OR (SELECT removed_from_source_at FROM records WHERE id = ?) IS NULL THEN NULL
                ELSE removed_from_source_at
            END,
            updated_at = CURRENT_TIMESTAMP
        WHERE id = ?
        "#,
    )
    .bind(duplicate)
    .bind(duplicate)
    .bind(duplicate)
    .bind(duplicate)
    .bind(duplicate)
    .bind(duplicate)
    .bind(duplicate)
    .bind(keeper)
    .execute(pool)
    .await?;

    sqlx::query("UPDATE artifacts SET record_id = ? WHERE record_id = ?")
        .bind(keeper)
        .bind(duplicate)
        .execute(pool)
        .await?;
    sqlx::query("UPDATE record_assets SET record_id = ? WHERE record_id = ?")
        .bind(keeper)
        .bind(duplicate)
        .execute(pool)
        .await?;
    sqlx::query("UPDATE analysis_chunks SET record_id = ? WHERE record_id = ?")
        .bind(keeper)
        .bind(duplicate)
        .execute(pool)
        .await?;
    let _ = sqlx::query("UPDATE analysis_chunks_fts SET record_id = ? WHERE record_id = ?")
        .bind(keeper)
        .bind(duplicate)
        .execute(pool)
        .await;
    sqlx::query("UPDATE intelligence_fragments SET record_id = ? WHERE record_id = ?")
        .bind(keeper)
        .bind(duplicate)
        .execute(pool)
        .await?;
    sqlx::query("UPDATE record_forensics SET record_id = ? WHERE record_id = ?")
        .bind(keeper)
        .bind(duplicate)
        .execute(pool)
        .await?;
    sqlx::query("UPDATE intelligence_logs SET record_id = ? WHERE record_id = ?")
        .bind(keeper)
        .bind(duplicate)
        .execute(pool)
        .await?;
    sqlx::query("UPDATE download_job_items SET record_id = ? WHERE record_id = ?")
        .bind(keeper)
        .bind(duplicate)
        .execute(pool)
        .await?;

    sqlx::query(
        r#"
        INSERT OR IGNORE INTO analysis_results (record_id, ocr_text, status, processed_at)
        SELECT ?, ocr_text, status, processed_at FROM analysis_results WHERE record_id = ?
        "#,
    )
    .bind(keeper)
    .bind(duplicate)
    .execute(pool)
    .await?;
    sqlx::query("DELETE FROM analysis_results WHERE record_id = ?")
        .bind(duplicate)
        .execute(pool)
        .await?;

    sqlx::query(
        r#"
        INSERT OR IGNORE INTO record_entities (record_id, entity_id, confidence)
        SELECT ?, entity_id, confidence FROM record_entities WHERE record_id = ?
        "#,
    )
    .bind(keeper)
    .bind(duplicate)
    .execute(pool)
    .await?;
    sqlx::query("DELETE FROM record_entities WHERE record_id = ?")
        .bind(duplicate)
        .execute(pool)
        .await?;

    sqlx::query(
        r#"
        INSERT OR IGNORE INTO case_records (case_id, record_id, notes)
        SELECT case_id, ?, notes FROM case_records WHERE record_id = ?
        "#,
    )
    .bind(keeper)
    .bind(duplicate)
    .execute(pool)
    .await?;
    sqlx::query("DELETE FROM case_records WHERE record_id = ?")
        .bind(duplicate)
        .execute(pool)
        .await?;
    sqlx::query("UPDATE case_notes SET record_id = ? WHERE record_id = ?")
        .bind(keeper)
        .bind(duplicate)
        .execute(pool)
        .await?;

    sqlx::query("DELETE FROM records WHERE id = ?")
        .bind(duplicate)
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

fn normalize_source_url(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let absolute = if trimmed.starts_with('/') {
        format!("https://www.war.gov{trimmed}")
    } else {
        trimmed.to_string()
    };
    let parseable = absolute.replace(' ', "%20");

    match Url::parse(&parseable) {
        Ok(mut url) => {
            url.set_fragment(None);
            url.to_string()
        }
        Err(_) => trimmed.to_string(),
    }
}

fn canonical_document_identity(raw: &str) -> Option<String> {
    let normalized = normalize_source_url(raw);
    let url = Url::parse(&normalized).ok()?;
    let host = url.host_str()?.to_ascii_lowercase();
    let decoded_path = percent_decode_str(url.path()).decode_utf8_lossy();
    let path = normalize_identity_text(&decoded_path);
    Some(format!("{}://{}{}", url.scheme().to_ascii_lowercase(), host, path))
}

fn normalize_identity_text(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

fn source_asset_class(record: &CsvRecord) -> String {
    let has_document = record
        .document_url
        .as_deref()
        .map(|url| !url.trim().is_empty())
        .unwrap_or(false);
    let has_dvids = record
        .dvids_video_id
        .as_deref()
        .map(|id| !id.trim().is_empty())
        .unwrap_or(false);
    let doc_type = record
        .doc_type
        .as_deref()
        .unwrap_or("")
        .trim()
        .to_ascii_uppercase();

    match (doc_type.as_str(), has_document, has_dvids) {
        (_, true, true) => "document_with_dvids",
        ("VID", false, true) => "dvids_video",
        ("AUD", false, true) => "dvids_audio",
        ("IMG", true, false) => "image",
        (_, true, false) => "document",
        (_, false, true) => "dvids_media",
        _ => "metadata_only",
    }
    .to_string()
}

fn release_labels_for(records: &[ParsedOfficialRecord]) -> HashMap<String, String> {
    let mut dates: Vec<(chrono::NaiveDate, String)> = records
        .iter()
        .filter_map(|record| {
            let date = record.csv.release_date.as_ref()?;
            Some((parse_release_date(date)?, date.clone()))
        })
        .collect();
    dates.sort_by_key(|date| date.0);
    dates.dedup_by(|left, right| left.1 == right.1);

    dates
        .into_iter()
        .enumerate()
        .map(|(index, (_, raw_date))| (raw_date, format!("Release {:02}", index + 1)))
        .collect()
}

fn parse_release_date(raw: &str) -> Option<chrono::NaiveDate> {
    let parts: Vec<u32> = raw
        .split('/')
        .map(str::trim)
        .map(str::parse)
        .collect::<std::result::Result<Vec<u32>, _>>()
        .ok()?;
    if parts.len() != 3 {
        return None;
    }
    let year = if parts[2] < 70 {
        2000 + parts[2] as i32
    } else if parts[2] < 100 {
        1900 + parts[2] as i32
    } else {
        parts[2] as i32
    };
    chrono::NaiveDate::from_ymd_opt(year, parts[0], parts[1])
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
        let canonical_url =
            canonical_document_identity(url).unwrap_or_else(|| normalize_identity_text(url));
        format!("url:{}|title:{}", canonical_url, normalized_title)
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

#[cfg(test)]
mod tests {
    use super::*;

    const CURRENT_STYLE_CSV: &str = "\
Redaction,Release Date,Title,Type,Video Pairing,PDF Pairing,Description Blurb,DVIDS Video ID,Video Title,Agency,Incident Date,Incident Location,PDF | Image Link,Modal Image,Image Alt Text,Image VIRIN\n\
TRUE,5/22/26,\"DOW-UAP-PR050, \"\"4 UAP Formation Iran 26 Aug 2022 over water [CALLSIGN]\"\"\",VID,,,Video row,1007706,Video title,Department of War,2022,CENTCOM,,,,\n\
FALSE,5/22/26,\"DOW-UAP-D017, UAP Reported at Sandia Base, 1948-1950\",PDF,,,PDF row,,Video title,Department of War,1948-1950,New Mexico,https://www.war.gov/medialink/ufo/052226/release_02/documents/DOW-UAP-D017_General_Correspondence_Of_Sandia.pdf,https://www.war.gov/medialink/ufo/052226/release_02/thumbnails/DOW-UAP-D017_General_Correspondence_Of_Sandia.jpg,Alt,260508-D-D0360-1053\n";

    #[test]
    fn parses_current_csv_media_metadata() {
        let records = parse_csv_records(CURRENT_STYLE_CSV.as_bytes()).expect("csv parses");

        assert_eq!(records.len(), 2);
        let video = records
            .iter()
            .find(|record| record.csv.title.as_deref().unwrap_or("").contains("PR050"))
            .expect("video record");
        assert_eq!(video.csv.dvids_video_id.as_deref(), Some("1007706"));
        assert_eq!(video.csv.doc_type.as_deref(), Some("VID"));

        let pdf = records
            .iter()
            .find(|record| record.csv.title.as_deref().unwrap_or("").contains("D017"))
            .expect("pdf record");
        assert_eq!(
            pdf.csv.modal_image.as_deref(),
            Some("https://www.war.gov/medialink/ufo/052226/release_02/thumbnails/DOW-UAP-D017_General_Correspondence_Of_Sandia.jpg")
        );
        assert_eq!(pdf.csv.image_virin.as_deref(), Some("260508-D-D0360-1053"));
    }

    #[test]
    fn stable_key_survives_url_encoding_changes() {
        let mut raw = CsvRecord {
            redaction: None,
            release_date: Some("5/8/26".to_string()),
            title: Some("18_100754_ General 1946-7_Vol_2".to_string()),
            doc_type: Some("PDF".to_string()),
            video_pairing: None,
            pdf_pairing: None,
            description: None,
            dvids_video_id: None,
            video_title: None,
            agency: Some("Department of War".to_string()),
            incident_date: None,
            incident_location: None,
            document_url: Some("https://www.war.gov/medialink/ufo/release_1/18_100754_ general 1946-7_vol_2.pdf".to_string()),
            modal_image: None,
            image_alt_text: None,
            image_virin: None,
        };
        let raw_key = stable_key(&raw);

        raw.document_url = Some(
            "https://www.war.gov/medialink/ufo/release_1/18_100754_%20general%201946-7_vol_2.pdf"
                .to_string(),
        );

        assert_eq!(raw_key, stable_key(&raw));
    }
}
