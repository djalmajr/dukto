use std::path::{Path, PathBuf};

use super::fs::{sanitize_name, validate_relative_path};

/// Resolve an incoming relative path beneath the selected receive directory.
///
/// The selected root itself may be a symlink, but existing directories below it may not be.
/// Parent directories are created one component at a time, then canonicalized and checked to
/// remain beneath the canonical root before the final path is returned.
pub async fn resolve_safe_destination_path(
    destination_root: &Path,
    relative_path: &str,
) -> Result<PathBuf, std::io::Error> {
    validate_relative_path(relative_path)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidInput, error))?;

    let sanitized = relative_path
        .split('/')
        .map(sanitize_name)
        .collect::<Vec<_>>()
        .join("/");
    let components = Path::new(&sanitized)
        .components()
        .map(|component| match component {
            std::path::Component::Normal(name) => Ok(name.to_os_string()),
            _ => Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Received path is not relative",
            )),
        })
        .collect::<Result<Vec<_>, _>>()?;
    let (file_name, parent_components) = components.split_last().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Received path does not contain a file name",
        )
    })?;

    // Canonicalizing the user-selected root intentionally accepts a root that is itself a
    // symlink. Only symlinks among the received relative path components are rejected.
    let canonical_root = tokio::fs::canonicalize(destination_root).await?;
    if !tokio::fs::metadata(&canonical_root).await?.is_dir() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotADirectory,
            "Selected destination is not a directory",
        ));
    }

    let mut canonical_parent = canonical_root.clone();
    for component in parent_components {
        let candidate = canonical_parent.join(component);
        match tokio::fs::symlink_metadata(&candidate).await {
            Ok(metadata) => {
                if is_symlink_or_reparse_point(&metadata) {
                    return Err(unsafe_received_path_error());
                }
                if !metadata.is_dir() {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::NotADirectory,
                        "Received path parent is not a directory",
                    ));
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                match tokio::fs::create_dir(&candidate).await {
                    Ok(()) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                    Err(error) => return Err(error),
                }

                let metadata = tokio::fs::symlink_metadata(&candidate).await?;
                if is_symlink_or_reparse_point(&metadata) {
                    return Err(unsafe_received_path_error());
                }
                if !metadata.is_dir() {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::NotADirectory,
                        "Received path parent is not a directory",
                    ));
                }
            }
            Err(error) => return Err(error),
        }

        canonical_parent = tokio::fs::canonicalize(candidate).await?;
        if !canonical_parent.starts_with(&canonical_root) {
            return Err(unsafe_received_path_error());
        }
    }

    Ok(canonical_parent.join(file_name))
}

fn is_symlink_or_reparse_point(metadata: &std::fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }

    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;

        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
        metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }

    #[cfg(not(windows))]
    {
        false
    }
}

fn unsafe_received_path_error() -> std::io::Error {
    std::io::Error::new(
        std::io::ErrorKind::PermissionDenied,
        "Received path traverses a symlink or escapes the selected destination",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_dir(suffix: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "dukto-safe-destination-{}-{}",
            suffix,
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[tokio::test]
    async fn safe_destination_creates_normal_nested_parents_inside_root() {
        // Mutation captured: skipping parent creation breaks valid nested receives.
        let root = temp_dir("nested");
        let destination = resolve_safe_destination_path(&root, "nested/deeper/payload.bin")
            .await
            .unwrap();
        let canonical_root = tokio::fs::canonicalize(&root).await.unwrap();

        assert_eq!(
            destination,
            canonical_root.join("nested/deeper/payload.bin")
        );
        assert!(destination.parent().unwrap().is_dir());
        assert!(destination.starts_with(&canonical_root));
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn safe_destination_rejects_symlink_parent_without_touching_external_files() {
        // Mutation captured: dropping symlink rejection permits writes outside the selected root.
        use std::os::unix::fs::symlink;

        let root = temp_dir("root");
        let outside = temp_dir("outside");
        let sentinel = outside.join("sentinel.txt");
        fs::write(&sentinel, b"keep this file").unwrap();
        symlink(&outside, root.join("escape")).unwrap();

        let error = resolve_safe_destination_path(&root, "escape/payload.bin")
            .await
            .unwrap_err();

        assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
        assert_eq!(fs::read(&sentinel).unwrap(), b"keep this file");
        assert!(!outside.join("payload.bin").exists());
        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(outside).unwrap();
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn safe_destination_allows_a_user_selected_root_that_is_a_symlink() {
        // Mutation captured: rejecting the selected root symlink breaks a valid user-selected destination.
        use std::os::unix::fs::symlink;

        let root = temp_dir("selected-root");
        let root_name = root.file_name().unwrap().to_string_lossy();
        let selected_root = root.with_file_name(format!("{root_name}-link"));
        symlink(&root, &selected_root).unwrap();

        let destination = resolve_safe_destination_path(&selected_root, "nested/payload.bin")
            .await
            .unwrap();
        let canonical_root = tokio::fs::canonicalize(&selected_root).await.unwrap();

        assert_eq!(destination, canonical_root.join("nested/payload.bin"));
        assert!(destination.parent().unwrap().is_dir());
        fs::remove_file(selected_root).unwrap();
        fs::remove_dir_all(root).unwrap();
    }
}
