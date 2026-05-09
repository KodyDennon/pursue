use anyhow::{anyhow, Result};
use sqlx::{Row, SqlitePool};
use std::path::PathBuf;
use tokio::fs;
use uuid::Uuid;

use crate::cases;
use crate::library::LibraryManager;
use crate::models::{ExportCaseRequest, ExportResult};

pub async fn export_case(
    pool: &SqlitePool,
    library: &LibraryManager,
    request: ExportCaseRequest,
) -> Result<ExportResult> {
    let format = request.format.to_ascii_lowercase();
    if format != "markdown" && format != "html" {
        return Err(anyhow!("export format must be `markdown` or `html`"));
    }

    let case = cases::get_case(pool, &request.case_id).await?;
    let records = cases::case_records(pool, &request.case_id).await?;
    let notes = cases::case_notes(pool, &request.case_id).await?;
    let export_id = Uuid::new_v4().to_string();
    let case_dir = library.exports_dir().join(&request.case_id);
    fs::create_dir_all(&case_dir).await?;

    let markdown = build_markdown(pool, &case, &records, &notes).await?;
    let (relative_path, absolute_path) = if format == "markdown" {
        let path = case_dir.join(format!("{export_id}.md"));
        fs::write(&path, markdown).await?;
        (relative_export_path(&path, library), path)
    } else {
        let path = case_dir.join(format!("{export_id}.html"));
        fs::write(&path, markdown_to_html(&markdown, &case.title)).await?;
        (relative_export_path(&path, library), path)
    };

    sqlx::query("INSERT INTO exports (id, case_id, format, relative_path, created_at) VALUES (?, ?, ?, ?, ?)")
        .bind(&export_id)
        .bind(&request.case_id)
        .bind(&format)
        .bind(&relative_path)
        .bind(now())
        .execute(pool)
        .await?;

    Ok(ExportResult {
        export_id,
        case_id: request.case_id,
        format,
        relative_path,
        absolute_path: absolute_path.to_string_lossy().into_owned(),
    })
}

async fn build_markdown(
    pool: &SqlitePool,
    case: &crate::models::CaseSummary,
    records: &[sqlx::sqlite::SqliteRow],
    notes: &[sqlx::sqlite::SqliteRow],
) -> Result<String> {
    let mut output = String::new();
    output.push_str(&format!("# {}\n\n", case.title));
    if let Some(description) = &case.description {
        output.push_str(description);
        output.push_str("\n\n");
    }
    output.push_str("## Case Summary\n\n");
    output.push_str(&format!("- Case ID: `{}`\n", case.id));
    output.push_str(&format!("- Created: `{}`\n", case.created_at));
    output.push_str(&format!("- Evidence records: `{}`\n", records.len()));
    output.push_str(&format!("- Notes: `{}`\n\n", notes.len()));

    output.push_str("## Evidence\n\n");
    for record in records {
        let record_id = record.get::<String, _>("id");
        let title = record.get::<String, _>("title");
        output.push_str(&format!("### {}\n\n", title));
        output.push_str(&format!("- Record ID: `{}`\n", record_id));
        write_optional(
            &mut output,
            "Agency",
            record.get::<Option<String>, _>("agency"),
        );
        write_optional(
            &mut output,
            "Release Date",
            record.get::<Option<String>, _>("release_date"),
        );
        write_optional(
            &mut output,
            "Incident Date",
            record.get::<Option<String>, _>("incident_date"),
        );
        write_optional(
            &mut output,
            "Incident Location",
            record.get::<Option<String>, _>("incident_location"),
        );
        write_optional(
            &mut output,
            "Source URL",
            record.get::<Option<String>, _>("document_url"),
        );
        write_optional(
            &mut output,
            "Local Path",
            record.get::<Option<String>, _>("local_path"),
        );
        write_optional(
            &mut output,
            "Case Notes",
            record.get::<Option<String>, _>("case_record_notes"),
        );

        if let Some(summary) = record.get::<Option<String>, _>("summary") {
            output.push_str("\n");
            output.push_str(summary.trim());
            output.push_str("\n");
        }

        let entities = sqlx::query(
            r#"
            SELECT e.name, e.entity_type, re.confidence
            FROM entities e
            JOIN record_entities re ON re.entity_id = e.id
            WHERE re.record_id = ?
            ORDER BY e.entity_type, e.name
            "#,
        )
        .bind(&record_id)
        .fetch_all(pool)
        .await?;
        if !entities.is_empty() {
            output.push_str("\nEntities:\n");
            for entity in entities {
                output.push_str(&format!(
                    "- {} `{}` ({:.2})\n",
                    entity.get::<String, _>("entity_type"),
                    entity.get::<String, _>("name"),
                    entity.get::<f64, _>("confidence")
                ));
            }
        }

        let analysis = sqlx::query_scalar::<_, String>(
            "SELECT ocr_text FROM analysis_results WHERE record_id = ?",
        )
        .bind(&record_id)
        .fetch_optional(pool)
        .await?;
        if let Some(analysis) = analysis.filter(|text| !text.trim().is_empty()) {
            output.push_str("\nAnalysis Excerpt:\n\n");
            output.push_str("> ");
            output.push_str(
                &analysis
                    .replace('\n', "\n> ")
                    .chars()
                    .take(1600)
                    .collect::<String>(),
            );
            output.push_str("\n\n");
        } else {
            output.push_str("\n");
        }
    }

    output.push_str("## Investigator Notes\n\n");
    if notes.is_empty() {
        output.push_str("No investigator notes have been added.\n");
    } else {
        for note in notes {
            output.push_str(&format!(
                "- `{}`: {}\n",
                note.get::<String, _>("created_at"),
                note.get::<String, _>("body")
            ));
        }
    }

    Ok(output)
}

fn markdown_to_html(markdown: &str, title: &str) -> String {
    let mut body = String::new();
    for line in markdown.lines() {
        if let Some(text) = line.strip_prefix("# ") {
            body.push_str(&format!("<h1>{}</h1>\n", escape_html(text)));
        } else if let Some(text) = line.strip_prefix("## ") {
            body.push_str(&format!("<h2>{}</h2>\n", escape_html(text)));
        } else if let Some(text) = line.strip_prefix("### ") {
            body.push_str(&format!("<h3>{}</h3>\n", escape_html(text)));
        } else if let Some(text) = line.strip_prefix("- ") {
            body.push_str(&format!("<p class=\"bullet\">{}</p>\n", escape_html(text)));
        } else if let Some(text) = line.strip_prefix("> ") {
            body.push_str(&format!("<blockquote>{}</blockquote>\n", escape_html(text)));
        } else if line.trim().is_empty() {
            body.push('\n');
        } else {
            body.push_str(&format!("<p>{}</p>\n", escape_html(line)));
        }
    }

    format!(
        r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <title>{}</title>
  <style>
    body {{ font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif; max-width: 980px; margin: 48px auto; padding: 0 24px; color: #18181b; line-height: 1.55; }}
    h1, h2, h3 {{ line-height: 1.15; }}
    h1 {{ border-bottom: 3px solid #18181b; padding-bottom: 16px; }}
    h2 {{ margin-top: 40px; border-bottom: 1px solid #d4d4d8; padding-bottom: 8px; }}
    code {{ background: #f4f4f5; padding: 2px 4px; border-radius: 4px; }}
    blockquote {{ border-left: 4px solid #2563eb; background: #f8fafc; padding: 10px 14px; white-space: pre-wrap; }}
    .bullet {{ margin: 4px 0 4px 24px; }}
  </style>
</head>
<body>
{}
</body>
</html>"#,
        escape_html(title),
        body
    )
}

fn write_optional(output: &mut String, label: &str, value: Option<String>) {
    if let Some(value) = value.filter(|value| !value.trim().is_empty()) {
        output.push_str(&format!("- {label}: `{}`\n", value.trim()));
    }
}

fn relative_export_path(path: &PathBuf, library: &LibraryManager) -> String {
    path.strip_prefix(library.exports_dir())
        .unwrap_or(path)
        .to_string_lossy()
        .into_owned()
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}
