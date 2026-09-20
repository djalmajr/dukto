use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;

/// File bytes are written beside their destination under an unmistakable temporary name.
/// Dropping this guard removes an incomplete transfer; successful callers atomically publish it.
pub struct PartialFile {
    file: Option<tokio::fs::File>,
    path: PathBuf,
}

impl PartialFile {
    pub fn file_mut(&mut self) -> &mut tokio::fs::File {
        self.file
            .as_mut()
            .expect("partial file is open until commit")
    }

    /// Publish the completed bytes without replacing any existing destination.
    pub async fn commit_to(mut self, requested_path: &Path) -> Result<PathBuf, std::io::Error> {
        let mut file = self.file.take().expect("partial file is open until commit");
        file.flush().await?;
        file.sync_all().await?;
        drop(file);

        for index in 0..=999 {
            let candidate = conflict_candidate(requested_path, index);
            match rename_no_replace(&self.path, &candidate).await {
                Ok(()) => return Ok(candidate),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        }

        for _ in 0..64 {
            let parent = requested_path.parent().unwrap_or(Path::new("."));
            let stem = requested_path
                .file_stem()
                .and_then(|name| name.to_str())
                .unwrap_or("file");
            let extension = requested_path.extension().and_then(|value| value.to_str());
            let suffix = uuid::Uuid::new_v4();
            let name = match extension {
                Some(extension) => format!("{stem}_{suffix}.{extension}"),
                None => format!("{stem}_{suffix}"),
            };
            let candidate = parent.join(name);
            match rename_no_replace(&self.path, &candidate).await {
                Ok(()) => return Ok(candidate),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        }

        Err(unique_destination_exhausted_error())
    }
}

impl Drop for PartialFile {
    fn drop(&mut self) {
        // Close first so cleanup also works on Windows, where open files cannot be unlinked.
        drop(self.file.take());
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Create a same-directory temporary file so final publication can be atomic and no-clobber.
pub async fn create_partial_file(path: &Path) -> Result<PartialFile, std::io::Error> {
    let parent = path.parent().unwrap_or(Path::new("."));
    #[cfg(windows)]
    let parent = tokio::fs::canonicalize(parent).await?;
    #[cfg(not(windows))]
    let parent = parent.to_path_buf();

    for _ in 0..64 {
        let partial_path = parent.join(format!(".dukto-partial-{}", uuid::Uuid::new_v4()));
        match tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&partial_path)
            .await
        {
            Ok(file) => {
                return Ok(PartialFile {
                    file: Some(file),
                    path: partial_path,
                });
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }

    Err(partial_file_exhausted_error())
}

fn unique_destination_exhausted_error() -> std::io::Error {
    std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        "Unable to publish a unique destination",
    )
}

fn partial_file_exhausted_error() -> std::io::Error {
    std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        "Unable to create a partial file in the selected destination",
    )
}

fn conflict_candidate(path: &Path, index: usize) -> PathBuf {
    if index == 0 {
        return path.to_path_buf();
    }

    let parent = path.parent().unwrap_or(Path::new("."));
    let stem = path
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("file");
    let name = match path.extension().and_then(|value| value.to_str()) {
        Some(extension) => format!("{stem} ({index}).{extension}"),
        None => format!("{stem} ({index})"),
    };
    parent.join(name)
}

async fn rename_no_replace(source: &Path, destination: &Path) -> Result<(), std::io::Error> {
    let source = source.to_path_buf();
    let destination = destination.to_path_buf();
    tokio::task::spawn_blocking(move || rename_no_replace_sync(&source, &destination))
        .await
        .map_err(|error| std::io::Error::other(error.to_string()))?
}

#[cfg(target_os = "linux")]
fn rename_no_replace_sync(source: &Path, destination: &Path) -> Result<(), std::io::Error> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    const AT_FDCWD: i32 = -100;
    const RENAME_NOREPLACE: u32 = 1;

    let source = CString::new(source.as_os_str().as_bytes())
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidInput, "NUL in path"))?;
    let destination = CString::new(destination.as_os_str().as_bytes())
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidInput, "NUL in path"))?;
    // SAFETY: both C strings live through the syscall; AT_FDCWD selects the current directory,
    // and RENAME_NOREPLACE asks the kernel to fail instead of replacing an existing path.
    let result = unsafe {
        linux_renameat2(
            AT_FDCWD,
            source.as_ptr(),
            AT_FDCWD,
            destination.as_ptr(),
            RENAME_NOREPLACE,
        )
    };
    if result == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

#[cfg(target_os = "linux")]
unsafe extern "C" {
    #[link_name = "renameat2"]
    fn linux_renameat2(
        old_directory: i32,
        old_path: *const std::ffi::c_char,
        new_directory: i32,
        new_path: *const std::ffi::c_char,
        flags: u32,
    ) -> i32;
}

#[cfg(target_os = "android")]
fn rename_no_replace_sync(source: &Path, destination: &Path) -> Result<(), std::io::Error> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    const AT_FDCWD: i32 = -100;
    const RENAME_NOREPLACE: u32 = 1;

    let source = CString::new(source.as_os_str().as_bytes())
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidInput, "NUL in path"))?;
    let destination = CString::new(destination.as_os_str().as_bytes())
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidInput, "NUL in path"))?;

    // Android's libc only exports renameat2 from API 30, while Dukto supports API 24.
    // Calling the Linux syscall directly keeps the no-replace guarantee without raising
    // the minimum Android version or risking an ordinary rename overwriting another file.
    // SAFETY: both C strings live through the syscall and all arguments match renameat2(2).
    let result = unsafe {
        libc::syscall(
            libc::SYS_renameat2,
            AT_FDCWD,
            source.as_ptr(),
            AT_FDCWD,
            destination.as_ptr(),
            RENAME_NOREPLACE,
        )
    };
    if result == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

#[cfg(any(target_os = "ios", target_os = "macos"))]
fn rename_no_replace_sync(source: &Path, destination: &Path) -> Result<(), std::io::Error> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    const RENAME_EXCL: u32 = 0x0000_0004;

    let source = CString::new(source.as_os_str().as_bytes())
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidInput, "NUL in path"))?;
    let destination = CString::new(destination.as_os_str().as_bytes())
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidInput, "NUL in path"))?;
    // SAFETY: both C strings live through the call and RENAME_EXCL prevents replacement.
    let result = unsafe { apple_renamex_np(source.as_ptr(), destination.as_ptr(), RENAME_EXCL) };
    if result == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

#[cfg(any(target_os = "ios", target_os = "macos"))]
unsafe extern "C" {
    #[link_name = "renamex_np"]
    fn apple_renamex_np(
        old_path: *const std::ffi::c_char,
        new_path: *const std::ffi::c_char,
        flags: u32,
    ) -> i32;
}

#[cfg(windows)]
fn rename_no_replace_sync(source: &Path, destination: &Path) -> Result<(), std::io::Error> {
    use std::os::windows::ffi::OsStrExt;

    // Canonicalization produces the extended-length `\\?\` form, including `\\?\UNC\`
    // for network shares. `MoveFileW` otherwise fails for long paths because the receiver's
    // destination directory may not have been opened with an extended prefix.
    let source = std::fs::canonicalize(source)?;
    let destination_parent = destination.parent().unwrap_or(Path::new("."));
    let destination_parent = std::fs::canonicalize(destination_parent)?;
    let destination_name = destination.file_name().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Destination path must include a file name",
        )
    })?;
    let destination = destination_parent.join(destination_name);

    let source: Vec<u16> = source.as_os_str().encode_wide().chain(Some(0)).collect();
    let destination: Vec<u16> = destination
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();
    // SAFETY: the nul-terminated UTF-16 buffers live through the call. MoveFileW fails if the
    // destination already exists; unlike MoveFileExW it has no replace-existing option.
    let result = unsafe { windows_move_file(source.as_ptr(), destination.as_ptr()) };
    if result != 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

#[cfg(windows)]
#[link(name = "Kernel32")]
unsafe extern "system" {
    #[link_name = "MoveFileW"]
    fn windows_move_file(old_path: *const u16, new_path: *const u16) -> i32;
}

#[cfg(not(any(
    target_os = "android",
    target_os = "ios",
    target_os = "linux",
    target_os = "macos",
    windows
)))]
fn rename_no_replace_sync(_source: &Path, _destination: &Path) -> Result<(), std::io::Error> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "Atomic no-overwrite file publication is unavailable on this platform",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::Arc;

    fn temp_dir(suffix: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "dukto-partial-test-{}-{}",
            suffix,
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[tokio::test]
    async fn dropping_an_incomplete_partial_file_removes_only_its_own_temporary_data() {
        // Mutation captured: removing Drop cleanup leaves this incomplete file beside the destination.
        let dir = temp_dir("partial-cleanup");
        let destination = dir.join("payload.bin");
        fs::write(&destination, b"pre-existing").unwrap();

        let mut partial = create_partial_file(&destination).await.unwrap();
        let partial_path = partial.path.clone();
        partial
            .file_mut()
            .write_all(b"incomplete payload")
            .await
            .unwrap();

        assert!(partial_path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with(".dukto-partial-"));
        assert!(partial_path.exists());
        assert_eq!(fs::read(&destination).unwrap(), b"pre-existing");

        drop(partial);

        assert!(!partial_path.exists());
        assert_eq!(fs::read(&destination).unwrap(), b"pre-existing");
        fs::remove_dir_all(dir).unwrap();
    }

    #[tokio::test]
    async fn completed_partial_file_publishes_without_overwriting_existing_data() {
        // Mutation captured: replacing no-overwrite rename with ordinary rename destroys the original bytes.
        let dir = temp_dir("partial-commit");
        let destination = dir.join("payload.bin");
        fs::write(&destination, b"pre-existing").unwrap();

        let mut partial = create_partial_file(&destination).await.unwrap();
        let partial_path = partial.path.clone();
        partial
            .file_mut()
            .write_all(b"complete payload")
            .await
            .unwrap();
        let published = partial.commit_to(&destination).await.unwrap();

        assert_eq!(published, dir.join("payload (1).bin"));
        assert_eq!(fs::read(&destination).unwrap(), b"pre-existing");
        assert_eq!(fs::read(&published).unwrap(), b"complete payload");
        assert!(!partial_path.exists());
        fs::remove_dir_all(dir).unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn concurrent_partial_file_commits_never_replace_each_other() {
        use tokio::sync::Barrier;

        // Mutation captured: publishing with an overwriting rename makes concurrent same-name commits collide.
        let dir = temp_dir("partial-concurrent-commit");
        let requested_path = dir.join("shared.bin");
        let barrier = Arc::new(Barrier::new(2));
        let mut tasks = Vec::new();

        for payload in [vec![b'A'; 64 * 1024], vec![b'B'; 64 * 1024]] {
            let path = requested_path.clone();
            let barrier = barrier.clone();
            tasks.push(tokio::spawn(async move {
                let mut partial = create_partial_file(&path).await.unwrap();
                partial.file_mut().write_all(&payload).await.unwrap();
                barrier.wait().await;
                let published = partial.commit_to(&path).await.unwrap();
                (published, payload)
            }));
        }

        let first = tasks.remove(0).await.unwrap();
        let second = tasks.remove(0).await.unwrap();
        assert_ne!(first.0, second.0);
        assert_eq!(fs::read(&first.0).unwrap(), first.1);
        assert_eq!(fs::read(&second.0).unwrap(), second.1);
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 2);
        fs::remove_dir_all(dir).unwrap();
    }

    #[tokio::test]
    async fn partial_name_supports_a_maximum_length_destination_name() {
        // Mutation captured: including the destination basename in the temp name exceeds NAME_MAX.
        let dir = temp_dir("partial-long-name");
        let nested_dir = dir.join("a".repeat(100)).join("b".repeat(100));
        fs::create_dir_all(&nested_dir).unwrap();
        let long_name = format!("{}.bin", "x".repeat(240));
        let requested_path = nested_dir.join(long_name);

        let mut partial = create_partial_file(&requested_path).await.unwrap();
        assert!(partial.path.file_name().unwrap().to_string_lossy().len() < 255);
        partial
            .file_mut()
            .write_all(b"complete long-path payload")
            .await
            .unwrap();
        let published = partial.commit_to(&requested_path).await.unwrap();
        assert_eq!(published, requested_path);
        assert_eq!(fs::read(&published).unwrap(), b"complete long-path payload");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn exhaustion_errors_do_not_include_destination_paths() {
        assert_eq!(
            unique_destination_exhausted_error().to_string(),
            "Unable to publish a unique destination"
        );
        assert_eq!(
            partial_file_exhausted_error().to_string(),
            "Unable to create a partial file in the selected destination"
        );
    }
}
