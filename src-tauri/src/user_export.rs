use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::Manager;

const MAX_EXPORT_COLLISION_ATTEMPTS: u16 = 1_000;

pub fn write_download(
    app: &tauri::AppHandle,
    prefix: &str,
    extension: &str,
    contents: &[u8],
) -> Result<PathBuf, String> {
    validate_file_component(prefix, "export prefix")?;
    validate_file_component(extension, "export extension")?;

    let directory = app
        .path()
        .download_dir()
        .map_err(|_| "The Downloads folder is unavailable.".to_string())?;
    std::fs::create_dir_all(&directory)
        .map_err(|_| "The export directory could not be created.".to_string())?;

    write_new_export(&directory, prefix, extension, contents)
}

fn validate_file_component(value: &str, label: &str) -> Result<(), String> {
    if value.is_empty()
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        return Err(format!("Invalid {label}."));
    }
    Ok(())
}

fn write_new_export(
    directory: &Path,
    prefix: &str,
    extension: &str,
    contents: &[u8],
) -> Result<PathBuf, String> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    for collision in 0..MAX_EXPORT_COLLISION_ATTEMPTS {
        let suffix = if collision == 0 {
            String::new()
        } else {
            format!("-{collision}")
        };
        let path = directory.join(format!("{prefix}-{timestamp}{suffix}.{extension}"));
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut file) => {
                file.write_all(contents)
                    .and_then(|_| file.sync_all())
                    .map_err(|_| "The export could not be written.".to_string())?;
                return Ok(path);
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(_) => return Err("The export could not be created.".to_string()),
        }
    }

    Err("Too many exports share the same name. Try again in a moment.".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_a_new_file_without_overwriting_an_existing_export() {
        let directory = tempfile::tempdir().unwrap();
        let first =
            write_new_export(directory.path(), "famvoice-history", "txt", b"first").unwrap();
        let second =
            write_new_export(directory.path(), "famvoice-history", "txt", b"second").unwrap();

        assert_ne!(first, second);
        assert_eq!(std::fs::read(first).unwrap(), b"first");
        assert_eq!(std::fs::read(second).unwrap(), b"second");
    }

    #[test]
    fn rejects_path_separators_and_untrusted_file_components() {
        assert!(validate_file_component("../history", "prefix").is_err());
        assert!(validate_file_component("JSON", "extension").is_err());
        assert!(validate_file_component("famvoice-diagnostics", "prefix").is_ok());
    }
}
