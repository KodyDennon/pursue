use anyhow::{anyhow, Result};
use std::path::{Path, PathBuf};
use tauri::Manager;

/// Name of the pointer file that redirects storage to a user-chosen directory.
/// It always lives in the platform-default app config dir so the app can find
/// it before any other storage is resolved.
const POINTER_FILE: &str = "storage-root.json";

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct StoragePointer {
    storage_root: String,
}

pub fn pointer_path<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Result<PathBuf> {
    Ok(app.path().app_config_dir()?.join(POINTER_FILE))
}

pub fn default_storage_root<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Result<PathBuf> {
    Ok(app.path().app_data_dir()?)
}

/// The storage root the user configured, if any. Returns the raw configured
/// path without checking whether it is currently reachable.
pub fn configured_storage_root<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Option<PathBuf> {
    let pointer = pointer_path(app).ok()?;
    let raw = std::fs::read_to_string(pointer).ok()?;
    let parsed: StoragePointer = serde_json::from_str(&raw).ok()?;
    let trimmed = parsed.storage_root.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(PathBuf::from(trimmed))
}

/// Effective storage root: the user-configured directory when present and
/// reachable, otherwise the platform-default app data directory. Falling back
/// (rather than failing startup) keeps the app usable when a secondary drive
/// is temporarily unavailable; the settings UI surfaces the mismatch.
pub fn resolve_storage_root<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Result<PathBuf> {
    let default_root = default_storage_root(app)?;
    let Some(custom) = configured_storage_root(app) else {
        return Ok(default_root);
    };
    if std::fs::create_dir_all(&custom).is_ok() {
        Ok(custom)
    } else {
        log::warn!(
            "configured storage root {} is not reachable; falling back to {}",
            custom.display(),
            default_root.display()
        );
        Ok(default_root)
    }
}

pub fn write_pointer<R: tauri::Runtime>(app: &tauri::AppHandle<R>, root: &Path) -> Result<()> {
    let pointer = pointer_path(app)?;
    if let Some(parent) = pointer.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let payload = serde_json::to_string_pretty(&StoragePointer {
        storage_root: root.to_string_lossy().into_owned(),
    })?;
    std::fs::write(pointer, payload)?;
    Ok(())
}

pub fn clear_pointer<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Result<()> {
    let pointer = pointer_path(app)?;
    if pointer.exists() {
        std::fs::remove_file(pointer)?;
    }
    Ok(())
}

/// Validates a prospective storage root before switching to it: it must be a
/// real directory we can create files in, must not be a filesystem root, and
/// must not be nested inside the current root (or vice versa) so migration
/// can never copy a tree into itself.
pub fn validate_new_root(new_root: &Path, current_root: &Path) -> Result<()> {
    if new_root.parent().is_none() {
        return Err(anyhow!(
            "pick a folder rather than a drive root (for example E:\\PursueData instead of E:\\)"
        ));
    }
    std::fs::create_dir_all(new_root).map_err(|e| {
        anyhow!(
            "cannot create storage directory {}: {e}",
            new_root.display()
        )
    })?;

    let probe = new_root.join(".pursue-write-probe");
    std::fs::write(&probe, b"probe").map_err(|e| {
        anyhow!(
            "storage directory {} is not writable: {e}",
            new_root.display()
        )
    })?;
    let _ = std::fs::remove_file(&probe);

    let canonical_new = std::fs::canonicalize(new_root)?;
    let canonical_current =
        std::fs::canonicalize(current_root).unwrap_or_else(|_| current_root.to_path_buf());
    if canonical_new == canonical_current {
        return Err(anyhow!(
            "the selected folder is already the active storage location"
        ));
    }
    if canonical_new.starts_with(&canonical_current) {
        return Err(anyhow!(
            "the new storage folder cannot be inside the current storage folder"
        ));
    }
    if canonical_current.starts_with(&canonical_new) {
        return Err(anyhow!(
            "the new storage folder cannot contain the current storage folder"
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, Default, serde::Serialize)]
pub struct MigrationStats {
    pub files_copied: u64,
    pub bytes_copied: u64,
    pub bytes_total: u64,
}

/// Copies all persisted data from one storage root to another, reporting
/// progress as (bytes_copied, bytes_total). The pointer file and purely
/// transient directories are skipped; originals are left in place so the old
/// location stays intact until the user verifies the move.
pub fn migrate_storage(
    from: &Path,
    to: &Path,
    mut progress: impl FnMut(u64, u64),
) -> Result<MigrationStats> {
    // download-parts and decrypted-cache are rebuildable scratch data; copying
    // them would only slow down a migration that already spans drives.
    const SKIP_NAMES: [&str; 3] = [POINTER_FILE, "download-parts", "decrypted-cache"];

    let should_skip = |path: &Path| -> bool {
        path.file_name()
            .and_then(|name| name.to_str())
            .map(|name| SKIP_NAMES.contains(&name))
            .unwrap_or(false)
    };

    let bytes_total = dir_size(from, &should_skip)?;
    let mut stats = MigrationStats {
        bytes_total,
        ..Default::default()
    };
    copy_tree(from, to, &should_skip, &mut stats, &mut progress)?;
    Ok(stats)
}

fn dir_size(dir: &Path, should_skip: &impl Fn(&Path) -> bool) -> Result<u64> {
    let mut total = 0;
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if should_skip(&path) {
            continue;
        }
        let metadata = entry.metadata()?;
        if metadata.is_dir() {
            total += dir_size(&path, should_skip)?;
        } else {
            total += metadata.len();
        }
    }
    Ok(total)
}

fn copy_tree(
    from: &Path,
    to: &Path,
    should_skip: &impl Fn(&Path) -> bool,
    stats: &mut MigrationStats,
    progress: &mut impl FnMut(u64, u64),
) -> Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let source = entry.path();
        if should_skip(&source) {
            continue;
        }
        let target = to.join(entry.file_name());
        if entry.metadata()?.is_dir() {
            copy_tree(&source, &target, should_skip, stats, progress)?;
        } else {
            let copied = std::fs::copy(&source, &target).map_err(|e| {
                anyhow!(
                    "failed to copy {} to {}: {e}",
                    source.display(),
                    target.display()
                )
            })?;
            stats.files_copied += 1;
            stats.bytes_copied += copied;
            progress(stats.bytes_copied, stats.bytes_total);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "pursue-storage-test-{name}-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn rejects_nested_roots() {
        let current = temp_dir("current");
        let nested = current.join("nested");
        assert!(validate_new_root(&nested, &current).is_err());

        let parent = temp_dir("parent");
        let inner = parent.join("inner");
        std::fs::create_dir_all(&inner).unwrap();
        assert!(validate_new_root(&parent, &inner).is_err());

        let _ = std::fs::remove_dir_all(&current);
        let _ = std::fs::remove_dir_all(&parent);
    }

    #[test]
    fn migrates_files_and_skips_transient_dirs() {
        let from = temp_dir("from");
        let to = temp_dir("to");

        std::fs::create_dir_all(from.join("library/ab")).unwrap();
        std::fs::write(from.join("library/ab/artifact.pdf"), b"data").unwrap();
        std::fs::write(from.join("pursue.db"), b"database").unwrap();
        std::fs::create_dir_all(from.join("download-parts")).unwrap();
        std::fs::write(from.join("download-parts/junk.tmp"), b"junk").unwrap();
        std::fs::write(from.join(POINTER_FILE), b"{}").unwrap();

        let stats = migrate_storage(&from, &to, |_, _| {}).unwrap();

        assert_eq!(stats.files_copied, 2);
        assert!(to.join("library/ab/artifact.pdf").exists());
        assert!(to.join("pursue.db").exists());
        assert!(!to.join("download-parts").exists());
        assert!(!to.join(POINTER_FILE).exists());
        // Originals stay in place until the user removes them.
        assert!(from.join("pursue.db").exists());

        let _ = std::fs::remove_dir_all(&from);
        let _ = std::fs::remove_dir_all(&to);
    }
}
