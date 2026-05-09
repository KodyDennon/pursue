use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct Record {
    pub id: String,
    pub title: String,
    pub agency: Option<String>,
    pub release_date: Option<String>,
    pub incident_date: Option<String>,
    pub incident_location: Option<String>,
    pub document_url: Option<String>,
    pub local_path: Option<String>,
    pub file_type: Option<String>,
    pub source_type: String,
    pub summary: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CsvRecord {
    #[serde(rename = "Redaction")]
    pub redaction: Option<String>,
    #[serde(rename = "Release Date")]
    pub release_date: Option<String>,
    #[serde(rename = "Title")]
    pub title: String,
    #[serde(rename = "Type")]
    pub doc_type: Option<String>,
    #[serde(rename = "Agency")]
    pub agency: Option<String>,
    #[serde(rename = "Incident Date")]
    pub incident_date: Option<String>,
    #[serde(rename = "Incident Location")]
    pub incident_location: Option<String>,
    #[serde(rename = "PDF | Image Link")]
    pub document_url: Option<String>,
    #[serde(rename = "Description Blurb")]
    pub description: Option<String>,
}
