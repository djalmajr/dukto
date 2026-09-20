use std::path::{Path, PathBuf};

use crate::protocol::types::ItemKind;

const MAX_DEPTH: usize = 100;

/// A single item from a directory walk.
#[derive(Debug, Clone)]
pub struct WalkItem {
    pub absolute_path: PathBuf,
    pub relative_path: String,
    pub kind: ItemKind,
    pub size: u64,
}

/// Walk a directory recursively and return a flat list of items.
/// Symlinks are skipped. Max depth is 100.
pub fn walk_directory(root: &Path) -> Result<Vec<WalkItem>, Box<dyn std::error::Error>> {
    let mut items = Vec::new();
    walk_recursive(root, root, 0, &mut items)?;
    Ok(items)
}

fn walk_recursive(
    root: &Path,
    current: &Path,
    depth: usize,
    items: &mut Vec<WalkItem>,
) -> Result<(), Box<dyn std::error::Error>> {
    if depth > MAX_DEPTH {
        return Err(format!("Directory depth exceeds {}", MAX_DEPTH).into());
    }

    let mut entries: Vec<_> = std::fs::read_dir(current)?.filter_map(|e| e.ok()).collect();
    entries.sort_by_key(|e| e.file_name());

    let mut has_children = false;

    for entry in entries {
        let path = entry.path();
        let metadata = entry.metadata()?;

        // Skip symlinks
        if metadata.is_symlink() {
            continue;
        }

        let rel = path
            .strip_prefix(root)
            .map_err(|e| format!("strip_prefix failed: {}", e))?
            .to_string_lossy()
            .replace('\\', "/");

        if metadata.is_file() {
            has_children = true;
            items.push(WalkItem {
                absolute_path: path,
                relative_path: rel,
                kind: ItemKind::File,
                size: metadata.len(),
            });
        } else if metadata.is_dir() {
            has_children = true;
            // Check if dir is empty or has contents
            let child_count_before = items.len();
            walk_recursive(root, &path, depth + 1, items)?;
            let child_count_after = items.len();

            // If dir had no file children, add it as an empty dir item
            if child_count_after == child_count_before {
                items.push(WalkItem {
                    absolute_path: path,
                    relative_path: rel,
                    kind: ItemKind::Directory,
                    size: 0,
                });
            }
        }
    }

    // If root-level dir itself is empty, add it
    if depth == 0 && !has_children {
        let rel = ".".to_string();
        items.push(WalkItem {
            absolute_path: root.to_path_buf(),
            relative_path: rel,
            kind: ItemKind::Directory,
            size: 0,
        });
    }

    Ok(())
}

/// Validate that a relative path is safe (no traversal, not absolute).
pub fn validate_relative_path(path: &str) -> Result<(), String> {
    if path.is_empty() {
        return Err("Empty relative path".into());
    }
    if path.starts_with('/') || path.starts_with('\\') {
        return Err("Absolute paths are not allowed".into());
    }
    if path.contains("..") {
        return Err("Path traversal is not allowed".into());
    }
    if path.contains('\0') {
        return Err("Null byte in path".into());
    }
    Ok(())
}

/// Sanitize a file/directory name for the current platform.
pub fn sanitize_name(name: &str) -> String {
    let mut sanitized = name.to_string();

    // Replace characters invalid on any major platform
    let invalid_chars = ['<', '>', ':', '"', '|', '?', '*'];
    for ch in &invalid_chars {
        sanitized = sanitized.replace(*ch, "_");
    }

    // Replace control characters
    sanitized = sanitized
        .chars()
        .map(|c| if c.is_control() { '_' } else { c })
        .collect();

    // Windows reserved names
    let reserved = [
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
        "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];
    let name_upper = sanitized.split('.').next().unwrap_or("").to_uppercase();
    if reserved.contains(&name_upper.as_str()) {
        sanitized = format!("_{}", sanitized);
    }

    // Trim trailing dots and spaces (Windows issue)
    sanitized = sanitized.trim_end_matches(['.', ' ']).to_string();

    if sanitized.is_empty() {
        sanitized = "_unnamed".to_string();
    }

    sanitized
}

/// Resolve a conflict by appending (1), (2), etc. to the stem.
/// Returns a path that does not yet exist.
pub fn resolve_conflict(path: &Path) -> PathBuf {
    if !path.exists() {
        return path.to_path_buf();
    }

    let parent = path.parent().unwrap_or(Path::new("."));
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("file");
    let ext = path.extension().and_then(|e| e.to_str());

    for i in 1..=999 {
        let new_name = match ext {
            Some(e) => format!("{} ({}).{}", stem, i, e),
            None => format!("{} ({})", stem, i),
        };
        let candidate = parent.join(&new_name);
        if !candidate.exists() {
            return candidate;
        }
    }

    // Fallback: UUID suffix
    let new_name = match ext {
        Some(e) => format!("{}_{}.{}", stem, uuid::Uuid::new_v4(), e),
        None => format!("{}_{}", stem, uuid::Uuid::new_v4()),
    };
    parent.join(&new_name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_dir(suffix: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("duto-fs-test-{}-{}", suffix, uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    // --- walk_directory ---

    #[test]
    fn walk_directory_flat_files() {
        let dir = temp_dir("walk-flat");
        fs::write(dir.join("a.txt"), "aaa").unwrap();
        fs::write(dir.join("b.txt"), "bbb").unwrap();

        let items = walk_directory(&dir).unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].relative_path, "a.txt");
        assert_eq!(items[0].kind, ItemKind::File);
        assert_eq!(items[0].size, 3);
        assert_eq!(items[1].relative_path, "b.txt");

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn walk_directory_nested() {
        let dir = temp_dir("walk-nested");
        fs::create_dir_all(dir.join("sub")).unwrap();
        fs::write(dir.join("root.txt"), "r").unwrap();
        fs::write(dir.join("sub/inner.txt"), "i").unwrap();

        let items = walk_directory(&dir).unwrap();
        assert_eq!(items.len(), 2);

        let paths: Vec<&str> = items.iter().map(|i| i.relative_path.as_str()).collect();
        assert!(paths.contains(&"root.txt"));
        assert!(paths.contains(&"sub/inner.txt"));

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn walk_directory_empty_dir_included() {
        let dir = temp_dir("walk-empty");
        fs::create_dir_all(dir.join("empty_sub")).unwrap();
        fs::write(dir.join("file.txt"), "f").unwrap();

        let items = walk_directory(&dir).unwrap();
        assert_eq!(items.len(), 2);

        let empty = items
            .iter()
            .find(|i| i.kind == ItemKind::Directory)
            .unwrap();
        assert_eq!(empty.relative_path, "empty_sub");
        assert_eq!(empty.size, 0);

        fs::remove_dir_all(&dir).ok();
    }

    // --- validate_relative_path ---

    #[test]
    fn validate_path_accepts_normal() {
        assert!(validate_relative_path("file.txt").is_ok());
        assert!(validate_relative_path("sub/file.txt").is_ok());
        assert!(validate_relative_path("a/b/c/d.txt").is_ok());
    }

    #[test]
    fn validate_path_rejects_traversal() {
        assert!(validate_relative_path("../secret.txt").is_err());
        assert!(validate_relative_path("sub/../../etc/passwd").is_err());
        assert!(validate_relative_path("..").is_err());
    }

    #[test]
    fn validate_path_rejects_absolute() {
        assert!(validate_relative_path("/etc/passwd").is_err());
        assert!(validate_relative_path("\\Windows\\System32").is_err());
    }

    #[test]
    fn validate_path_rejects_empty_and_null() {
        assert!(validate_relative_path("").is_err());
        assert!(validate_relative_path("file\0.txt").is_err());
    }

    #[test]
    fn validation_errors_do_not_echo_received_paths() {
        let sentinel = "private-filename-sentinel";
        for path in [format!("/{sentinel}"), format!("../{sentinel}")] {
            let error = validate_relative_path(&path).unwrap_err();
            assert!(!error.contains(sentinel));
        }
    }

    // --- sanitize_name ---

    #[test]
    fn sanitize_name_removes_invalid_chars() {
        assert_eq!(sanitize_name("file<>:\"|?*.txt"), "file_______.txt");
    }

    #[test]
    fn sanitize_name_handles_reserved_windows() {
        assert_eq!(sanitize_name("CON"), "_CON");
        assert_eq!(sanitize_name("CON.txt"), "_CON.txt");
        assert_eq!(sanitize_name("nul"), "_nul");
        assert_eq!(sanitize_name("COM1"), "_COM1");
    }

    #[test]
    fn sanitize_name_trims_trailing_dots_spaces() {
        assert_eq!(sanitize_name("file."), "file");
        assert_eq!(sanitize_name("file..."), "file");
        assert_eq!(sanitize_name("file "), "file");
    }

    #[test]
    fn sanitize_name_empty_becomes_unnamed() {
        assert_eq!(sanitize_name(""), "_unnamed");
        assert_eq!(sanitize_name("..."), "_unnamed");
    }

    #[test]
    fn sanitize_name_normal_unchanged() {
        assert_eq!(sanitize_name("photo.jpg"), "photo.jpg");
        assert_eq!(sanitize_name("my-doc (1).pdf"), "my-doc (1).pdf");
    }

    // --- resolve_conflict ---

    #[test]
    fn resolve_conflict_no_conflict() {
        let dir = temp_dir("conflict-none");
        let path = dir.join("new.txt");
        assert_eq!(resolve_conflict(&path), path);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn resolve_conflict_renames_with_number() {
        let dir = temp_dir("conflict-rename");
        fs::write(dir.join("file.txt"), "existing").unwrap();

        let resolved = resolve_conflict(&dir.join("file.txt"));
        assert_eq!(
            resolved.file_name().unwrap().to_str().unwrap(),
            "file (1).txt"
        );

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn resolve_conflict_increments() {
        let dir = temp_dir("conflict-inc");
        fs::write(dir.join("file.txt"), "existing").unwrap();
        fs::write(dir.join("file (1).txt"), "also exists").unwrap();

        let resolved = resolve_conflict(&dir.join("file.txt"));
        assert_eq!(
            resolved.file_name().unwrap().to_str().unwrap(),
            "file (2).txt"
        );

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn resolve_conflict_no_extension() {
        let dir = temp_dir("conflict-noext");
        fs::write(dir.join("README"), "existing").unwrap();

        let resolved = resolve_conflict(&dir.join("README"));
        assert_eq!(
            resolved.file_name().unwrap().to_str().unwrap(),
            "README (1)"
        );

        fs::remove_dir_all(&dir).ok();
    }
}
