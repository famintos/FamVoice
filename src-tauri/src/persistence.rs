use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

static TEMP_FILE_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteOutcome {
    Written,
    Superseded,
}

/// A versioned atomic file. Callers reserve a revision while holding the lock
/// that owns their in-memory snapshot, then write after releasing that lock.
/// A late, older snapshot is ignored instead of replacing newer state.
pub struct AtomicFile {
    path: PathBuf,
    backup_path: PathBuf,
    next_revision: AtomicU64,
    written_revision: AtomicU64,
    write_lock: Mutex<()>,
}

impl AtomicFile {
    pub fn new(path: PathBuf) -> Self {
        let backup_path = sibling_path(&path, ".bak");
        Self {
            path,
            backup_path,
            next_revision: AtomicU64::new(1),
            written_revision: AtomicU64::new(0),
            write_lock: Mutex::new(()),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn backup_path(&self) -> &Path {
        &self.backup_path
    }

    pub fn reserve_revision(&self) -> u64 {
        self.next_revision.fetch_add(1, Ordering::Relaxed)
    }

    pub fn write(&self, revision: u64, data: &[u8]) -> Result<WriteOutcome, String> {
        let _guard = self
            .write_lock
            .lock()
            .map_err(|_| "Failed to lock persistent storage".to_string())?;

        if revision <= self.written_revision.load(Ordering::Acquire) {
            return Ok(WriteOutcome::Superseded);
        }

        atomic_write_with_backend(&SystemFileBackend, &self.path, data, true)
            .map_err(|error| format!("Failed to save {}: {error}", self.path.display()))?;
        self.written_revision.store(revision, Ordering::Release);
        Ok(WriteOutcome::Written)
    }

    /// Restore a validated recovery copy without replacing the recovery copy
    /// with the corrupt current file.
    pub fn restore_known_good(&self, data: &[u8]) -> Result<(), String> {
        let _guard = self
            .write_lock
            .lock()
            .map_err(|_| "Failed to lock persistent storage".to_string())?;
        atomic_write_with_backend(&SystemFileBackend, &self.path, data, false)
            .map_err(|error| format!("Failed to restore {}: {error}", self.path.display()))?;
        Ok(())
    }
}

pub fn preserve_corrupt_file(path: &Path) -> Result<PathBuf, String> {
    let mut destination = sibling_path(path, ".corrupt");
    if destination.exists() {
        let sequence = TEMP_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        destination = sibling_path(path, &format!(".corrupt.{sequence}"));
    }

    SystemFileBackend
        .copy_synced(path, &destination)
        .map_err(|error| format!("Failed to preserve corrupt {}: {error}", path.display()))?;
    Ok(destination)
}

fn sibling_path(path: &Path, suffix: &str) -> PathBuf {
    let mut file_name = path
        .file_name()
        .map(OsString::from)
        .unwrap_or_else(|| OsString::from("state"));
    file_name.push(suffix);
    path.with_file_name(file_name)
}

fn temporary_path(path: &Path, purpose: &str) -> PathBuf {
    let sequence = TEMP_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("state");
    path.with_file_name(format!(
        ".{file_name}.{}.{}.{}.tmp",
        std::process::id(),
        purpose,
        sequence
    ))
}

trait FileBackend {
    fn exists(&self, path: &Path) -> bool;
    fn write_new_synced(&self, path: &Path, data: &[u8]) -> io::Result<()>;
    fn copy_synced(&self, source: &Path, destination: &Path) -> io::Result<()>;
    fn replace(&self, source: &Path, destination: &Path) -> io::Result<()>;
    fn remove(&self, path: &Path);
}

struct SystemFileBackend;

impl SystemFileBackend {
    fn create_private_file(path: &Path) -> io::Result<File> {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);

        // Windows inherits the ACL of the per-user app-data directory. Unix is
        // not a supported release target, but use owner-only permissions when
        // compiling helpers there for development.
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }

        options.open(path)
    }
}

impl FileBackend for SystemFileBackend {
    fn exists(&self, path: &Path) -> bool {
        path.exists()
    }

    fn write_new_synced(&self, path: &Path, data: &[u8]) -> io::Result<()> {
        let mut file = Self::create_private_file(path)?;
        file.write_all(data)?;
        file.flush()?;
        file.sync_all()
    }

    fn copy_synced(&self, source: &Path, destination: &Path) -> io::Result<()> {
        let mut source_file = File::open(source)?;
        let mut destination_file = Self::create_private_file(destination)?;
        io::copy(&mut source_file, &mut destination_file)?;
        destination_file.flush()?;
        destination_file.sync_all()
    }

    fn replace(&self, source: &Path, destination: &Path) -> io::Result<()> {
        replace_file(source, destination)
    }

    fn remove(&self, path: &Path) {
        let _ = fs::remove_file(path);
    }
}

fn atomic_write_with_backend(
    backend: &dyn FileBackend,
    target: &Path,
    data: &[u8],
    preserve_current: bool,
) -> io::Result<()> {
    let parent = target.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "state file has no parent directory",
        )
    })?;
    fs::create_dir_all(parent)?;

    let pending_path = temporary_path(target, "pending");
    if let Err(error) = backend.write_new_synced(&pending_path, data) {
        backend.remove(&pending_path);
        return Err(error);
    }

    if preserve_current && backend.exists(target) {
        let backup_path = sibling_path(target, ".bak");
        let pending_backup_path = temporary_path(target, "backup");
        if let Err(error) = backend.copy_synced(target, &pending_backup_path) {
            backend.remove(&pending_backup_path);
            backend.remove(&pending_path);
            return Err(error);
        }
        if let Err(error) = backend.replace(&pending_backup_path, &backup_path) {
            backend.remove(&pending_backup_path);
            backend.remove(&pending_path);
            return Err(error);
        }
    }

    if let Err(error) = backend.replace(&pending_path, target) {
        backend.remove(&pending_path);
        return Err(error);
    }

    Ok(())
}

#[cfg(windows)]
fn replace_file(source: &Path, destination: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
    };

    let source_wide: Vec<u16> = source.as_os_str().encode_wide().chain(Some(0)).collect();
    let destination_wide: Vec<u16> = destination
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();
    let result = unsafe {
        MoveFileExW(
            source_wide.as_ptr(),
            destination_wide.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if result == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(not(windows))]
fn replace_file(source: &Path, destination: &Path) -> io::Result<()> {
    fs::rename(source, destination)?;
    if let Some(parent) = destination.parent() {
        File::open(parent)?.sync_all()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Barrier};
    use tempfile::tempdir;

    #[derive(Clone, Copy)]
    enum InjectedFailure {
        DiskFull,
        MidWrite,
    }

    struct FailingWriteBackend {
        failure: InjectedFailure,
    }

    impl FileBackend for FailingWriteBackend {
        fn exists(&self, path: &Path) -> bool {
            path.exists()
        }

        fn write_new_synced(&self, path: &Path, data: &[u8]) -> io::Result<()> {
            match self.failure {
                InjectedFailure::DiskFull => Err(io::Error::other("simulated disk full")),
                InjectedFailure::MidWrite => {
                    let mut file = SystemFileBackend::create_private_file(path)?;
                    file.write_all(&data[..data.len().min(3)])?;
                    file.flush()?;
                    Err(io::Error::other("simulated interrupted write"))
                }
            }
        }

        fn copy_synced(&self, source: &Path, destination: &Path) -> io::Result<()> {
            SystemFileBackend.copy_synced(source, destination)
        }

        fn replace(&self, source: &Path, destination: &Path) -> io::Result<()> {
            SystemFileBackend.replace(source, destination)
        }

        fn remove(&self, path: &Path) {
            SystemFileBackend.remove(path);
        }
    }

    #[test]
    fn atomic_write_replaces_existing_file_and_keeps_last_good_backup() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(&path, b"old-good").unwrap();

        atomic_write_with_backend(&SystemFileBackend, &path, b"new-good", true).unwrap();

        assert_eq!(fs::read(&path).unwrap(), b"new-good");
        assert_eq!(fs::read(sibling_path(&path, ".bak")).unwrap(), b"old-good");
    }

    #[test]
    fn disk_full_does_not_damage_existing_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("history.json");
        fs::write(&path, b"last-good").unwrap();

        let result = atomic_write_with_backend(
            &FailingWriteBackend {
                failure: InjectedFailure::DiskFull,
            },
            &path,
            b"new-state",
            true,
        );

        assert!(result.is_err());
        assert_eq!(fs::read(&path).unwrap(), b"last-good");
        assert!(dir.path().read_dir().unwrap().all(|entry| !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .ends_with(".tmp")));
    }

    #[test]
    fn interrupted_write_does_not_damage_existing_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("history.json");
        fs::write(&path, b"last-good").unwrap();

        let result = atomic_write_with_backend(
            &FailingWriteBackend {
                failure: InjectedFailure::MidWrite,
            },
            &path,
            b"new-state",
            true,
        );

        assert!(result.is_err());
        assert_eq!(fs::read(&path).unwrap(), b"last-good");
        assert!(dir.path().read_dir().unwrap().all(|entry| !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .ends_with(".tmp")));
    }

    #[test]
    fn a_late_old_revision_cannot_replace_a_newer_snapshot() {
        let dir = tempdir().unwrap();
        let file = Arc::new(AtomicFile::new(dir.path().join("history.json")));
        let old_revision = file.reserve_revision();
        let new_revision = file.reserve_revision();
        let new_written = Arc::new(Barrier::new(2));

        let old_file = Arc::clone(&file);
        let old_barrier = Arc::clone(&new_written);
        let old_writer = std::thread::spawn(move || {
            old_barrier.wait();
            old_file.write(old_revision, b"old-snapshot").unwrap()
        });

        let new_file = Arc::clone(&file);
        let new_barrier = Arc::clone(&new_written);
        let new_writer = std::thread::spawn(move || {
            let outcome = new_file.write(new_revision, b"new-snapshot").unwrap();
            new_barrier.wait();
            outcome
        });

        assert_eq!(new_writer.join().unwrap(), WriteOutcome::Written);
        assert_eq!(old_writer.join().unwrap(), WriteOutcome::Superseded);
        assert_eq!(fs::read(file.path()).unwrap(), b"new-snapshot");
    }
}
