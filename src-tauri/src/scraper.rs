use crate::models::CsvRecord;
use anyhow::Result;
use reqwest;
use csv::ReaderBuilder;
use sqlx::SqlitePool;
use uuid::Uuid;

const GOV_CSV_URL: &str = "https://www.war.gov/Portals/1/Interactive/2026/UFO/uap-csv.csv";

pub async fn sync_official_records(pool: &SqlitePool) -> Result<usize> {
    let client = reqwest::Client::new();
    let response = client.get(GOV_CSV_URL).send().await?.text().await?;

    let mut rdr = ReaderBuilder::new()
        .flexible(true)
        .has_headers(true)
        .from_reader(response.as_bytes());

    let mut count = 0;
    for result in rdr.deserialize() {
        let csv_rec: CsvRecord = result?;
        
        // Check if already exists by title and url
        let existing = sqlx::query(
            "SELECT id FROM records WHERE title = ? AND document_url = ?"
        )
        .bind(&csv_rec.title)
        .bind(&csv_rec.document_url)
        .fetch_optional(pool)
        .await?;

        if existing.is_none() {
            let id = Uuid::new_v4().to_string();
            sqlx::query(
                "INSERT INTO records (id, title, agency, release_date, incident_date, incident_location, document_url, file_type, source_type, summary)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
            )
            .bind(&id)
            .bind(&csv_rec.title)
            .bind(&csv_rec.agency)
            .bind(&csv_rec.release_date)
            .bind(&csv_rec.incident_date)
            .bind(&csv_rec.incident_location)
            .bind(&csv_rec.document_url)
            .bind(&csv_rec.doc_type)
            .bind("official")
            .bind(&csv_rec.description)
            .execute(pool)
            .await?;
            count += 1;
        }
    }

    Ok(count)
}
