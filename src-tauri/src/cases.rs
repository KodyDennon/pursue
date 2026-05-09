use anyhow::{anyhow, Result};
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

use crate::models::{
    AddRecordToCaseRequest, CaseNotesRequest, CaseSummary, CreateCaseRequest,
};

pub async fn list_cases(pool: &SqlitePool) -> Result<Vec<CaseSummary>> {
    Ok(sqlx::query_as::<_, CaseSummary>(
        r#"
        SELECT
            c.id,
            c.title,
            c.description,
            c.created_at,
            COUNT(DISTINCT cr.record_id) AS record_count,
            COUNT(DISTINCT cn.id) AS note_count
        FROM cases c
        LEFT JOIN case_records cr ON cr.case_id = c.id
        LEFT JOIN case_notes cn ON cn.case_id = c.id
        GROUP BY c.id
        ORDER BY c.created_at DESC
        "#,
    )
    .fetch_all(pool)
    .await?)
}

pub async fn create_case(pool: &SqlitePool, request: CreateCaseRequest) -> Result<CaseSummary> {
    let title = request.title.trim();
    if title.is_empty() {
        return Err(anyhow!("case title is required"));
    }
    let id = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO cases (id, title, description, created_at) VALUES (?, ?, ?, ?)")
        .bind(&id)
        .bind(title)
        .bind(request.description.as_deref().filter(|value| !value.trim().is_empty()))
        .bind(now())
        .execute(pool)
        .await?;

    get_case(pool, &id).await
}

pub async fn add_record_to_case(pool: &SqlitePool, request: AddRecordToCaseRequest) -> Result<()> {
    ensure_case(pool, &request.case_id).await?;
    let record_exists = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM records WHERE id = ?")
        .bind(&request.record_id)
        .fetch_one(pool)
        .await?;
    if record_exists == 0 {
        return Err(anyhow!("record not found: {}", request.record_id));
    }

    sqlx::query(
        r#"
        INSERT INTO case_records (case_id, record_id, notes)
        VALUES (?, ?, ?)
        ON CONFLICT(case_id, record_id) DO UPDATE SET notes = excluded.notes
        "#,
    )
    .bind(&request.case_id)
    .bind(&request.record_id)
    .bind(&request.notes)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn update_case_notes(pool: &SqlitePool, request: CaseNotesRequest) -> Result<()> {
    ensure_case(pool, &request.case_id).await?;
    if request.body.trim().is_empty() {
        return Err(anyhow!("note body is required"));
    }
    sqlx::query(
        r#"
        INSERT INTO case_notes (id, case_id, record_id, body, created_at, updated_at)
        VALUES (?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&request.case_id)
    .bind(&request.record_id)
    .bind(request.body.trim())
    .bind(now())
    .bind(now())
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get_case(pool: &SqlitePool, case_id: &str) -> Result<CaseSummary> {
    sqlx::query_as::<_, CaseSummary>(
        r#"
        SELECT
            c.id,
            c.title,
            c.description,
            c.created_at,
            COUNT(DISTINCT cr.record_id) AS record_count,
            COUNT(DISTINCT cn.id) AS note_count
        FROM cases c
        LEFT JOIN case_records cr ON cr.case_id = c.id
        LEFT JOIN case_notes cn ON cn.case_id = c.id
        WHERE c.id = ?
        GROUP BY c.id
        "#,
    )
    .bind(case_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| anyhow!("case not found: {case_id}"))
}

pub async fn case_records(pool: &SqlitePool, case_id: &str) -> Result<Vec<sqlx::sqlite::SqliteRow>> {
    ensure_case(pool, case_id).await?;
    Ok(sqlx::query(
        r#"
        SELECT r.*, cr.notes AS case_record_notes
        FROM case_records cr
        JOIN records r ON r.id = cr.record_id
        WHERE cr.case_id = ?
        ORDER BY COALESCE(r.incident_date, r.release_date, r.created_at) ASC
        "#,
    )
    .bind(case_id)
    .fetch_all(pool)
    .await?)
}

pub async fn case_notes(pool: &SqlitePool, case_id: &str) -> Result<Vec<sqlx::sqlite::SqliteRow>> {
    ensure_case(pool, case_id).await?;
    Ok(sqlx::query(
        "SELECT id, record_id, body, created_at, updated_at FROM case_notes WHERE case_id = ? ORDER BY created_at ASC",
    )
    .bind(case_id)
    .fetch_all(pool)
    .await?)
}

async fn ensure_case(pool: &SqlitePool, case_id: &str) -> Result<()> {
    let count = sqlx::query("SELECT COUNT(*) AS count FROM cases WHERE id = ?")
        .bind(case_id)
        .fetch_one(pool)
        .await?
        .get::<i64, _>("count");
    if count == 0 {
        return Err(anyhow!("case not found: {case_id}"));
    }
    Ok(())
}

fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}
