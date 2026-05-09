export interface Record {
  id: string;
  title: string;
  agency: string | null;
  release_date: string | null;
  incident_date: string | null;
  incident_location: string | null;
  document_url: string | null;
  local_path: string | null;
  file_type: string | null;
  source_type: string;
  summary: string | null;
}
