# Security Policy

## Supported Versions

Security fixes are applied to the latest published release and the `main` branch.

## Reporting A Vulnerability

Open a private security advisory on GitHub if available, or contact the repository owner directly before publishing details.

Please include:

- Affected version or commit.
- Operating system.
- Reproduction steps.
- Impact and affected data.

## Data Handling

PURSUE Data Analyzer is local-first. It stores evidence, snapshots, generated exports, and the SQLite database on the user's machine. Network access is used for official source sync and official evidence downloads.

Do not commit runtime databases, evidence files, source snapshots, generated exports, or local logs.

## Tauri Permissions

Changes to `src-tauri/capabilities/default.json` should be reviewed as security boundary changes. Keep permissions as narrow as the feature allows.
