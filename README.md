# PURSUE Data Analyzer

A professional-grade OSINT (Open Source Intelligence) and evidence management platform for declassified UAP/UFO data.

## 🛸 Project Vision
Built as a response to the "Presidential Unsealing and Reporting System for UAP Encounters" (PURSUE), this application transforms raw government data into a searchable, relational, and geospatially indexed intelligence portal. It is designed for deep investigation, maintaining data integrity against government redactions, and enabling portable case sharing.

## 🏗️ Architecture
- **Framework:** Tauri v2 (Rust Backend + Web Frontend)
- **Frontend:** Svelte 5 (Runes) + Tailwind CSS v4
- **Database:** SQLite (SQLx) for relational metadata + LanceDB for local Vector Search.
- **Geospatial:** OpenStreetMap (Leaflet) with offline tile caching.
- **Analysis Engine:** Rust-based concurrent scraper and indexing pipeline.

## 🚀 Getting Started
### Prerequisites
- [Bun](https://bun.sh/)
- [Rust](https://www.rust-lang.org/)
- Native dependencies (for OCR/Vectors): `brew install tesseract protobuf`

### Development
```bash
# Install dependencies
bun install

# Run the portal in development mode
bun tauri dev
```

## 📂 Repository Structure
- `src/`: Svelte 5 Frontend
  - `lib/components/`: Reusable UI modules (Map, ArchiveViewer, etc.)
  - `lib/types.ts`: Shared TypeScript interfaces.
- `src-tauri/`: Rust Core Engine
  - `src/db.rs`: SQLite initialization and migrations.
  - `src/scraper.rs`: Official government data ingestion logic.
  - `src/models.rs`: Rust data structures matching the database schema.
  - `migrations/`: Version-controlled SQL schema.

## 🛡️ Data Integrity
The portal uses an **Internal Library** approach. When you sync or ingest evidence, the files are indexed into a local managed library. This ensures that even if official files are modified or removed from government servers, your local investigation remains intact.
