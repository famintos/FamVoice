use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_HISTORY_ITEM_CHARS: usize = 10_000;

#[derive(Clone, Serialize, Deserialize)]
pub struct HistoryItem {
    pub id: u64,
    pub text: String,
    pub timestamp: u64,
}

pub struct HistoryState {
    pub items: Mutex<Vec<HistoryItem>>,
    pub path: PathBuf,
    next_id: AtomicU64,
}

impl HistoryState {
    pub fn load(app_dir: PathBuf) -> Self {
        let path = app_dir.join("history.json");
        let items = if path.exists() {
            match fs::read_to_string(&path) {
                Ok(data) => serde_json::from_str::<Vec<HistoryItem>>(&data).unwrap_or_else(|e| {
                    eprintln!(
                        "[FamVoice] Failed to parse history.json: {}, creating backup",
                        e
                    );
                    let backup_path = app_dir.join("history.json.corrupt");
                    let _ = fs::copy(&path, &backup_path);
                    Vec::new()
                }),
                Err(e) => {
                    eprintln!(
                        "[FamVoice] Failed to read history.json: {}, creating backup",
                        e
                    );
                    let backup_path = app_dir.join("history.json.corrupt");
                    let _ = fs::copy(&path, &backup_path);
                    Vec::new()
                }
            }
        } else {
            Vec::new()
        };

        let next_id = items
            .iter()
            .map(|item| item.id)
            .max()
            .unwrap_or(0)
            .saturating_add(1);

        Self {
            items: Mutex::new(items),
            path,
            next_id: AtomicU64::new(next_id),
        }
    }

    pub fn add(&self, text: String) {
        let serialized = {
            let mut items = self.items.lock().unwrap_or_else(|e| {
                eprintln!("[FamVoice] History lock poisoned in add(), recovering");
                e.into_inner()
            });
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;
            let id = self.next_id.fetch_add(1, Ordering::Relaxed);
            items.insert(
                0,
                HistoryItem {
                    id,
                    text: truncate_history_text(text),
                    timestamp,
                },
            );
            if items.len() > 100 {
                items.truncate(100);
            }
            serialize_items(&items)
        };
        // Lock is dropped here; file write happens outside the Mutex
        self.write_to_disk(serialized);
    }

    pub fn delete(&self, id: u64) {
        let serialized = {
            let mut items = self.items.lock().unwrap_or_else(|e| {
                eprintln!("[FamVoice] History lock poisoned in delete(), recovering");
                e.into_inner()
            });
            items.retain(|item| item.id != id);
            serialize_items(&items)
        };
        // Lock is dropped here; file write happens outside the Mutex
        self.write_to_disk(serialized);
    }

    pub fn clear(&self) {
        let serialized = {
            let mut items = self.items.lock().unwrap_or_else(|e| {
                eprintln!("[FamVoice] History lock poisoned in clear(), recovering");
                e.into_inner()
            });
            items.clear();
            serialize_items(&items)
        };
        // Lock is dropped here; file write happens outside the Mutex
        self.write_to_disk(serialized);
    }

    /// Write pre-serialized JSON to disk. Called after the Mutex lock is released
    /// so that file I/O does not block other threads waiting on the lock.
    fn write_to_disk(&self, serialized: Option<String>) {
        let Some(data) = serialized else { return };

        // Use OpenOptions for explicit control over file creation and write mode.
        // TODO: On Windows, restrict file permissions via platform-specific ACLs
        // (e.g., SetSecurityInfo / SECURITY_ATTRIBUTES) so that only the current
        // user can read history.json. Rust's std::fs permissions model maps to
        // Unix chmod bits and has no effect on Windows NTFS ACLs.
        let result = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&self.path)
            .and_then(|mut file| file.write_all(data.as_bytes()));

        if let Err(error) = result {
            eprintln!(
                "[FamVoice] Failed to write history to {}: {}",
                self.path.display(),
                error
            );
        }
    }
}

/// Serialize history items to a JSON string. Returns None on serialization failure.
fn serialize_items(items: &[HistoryItem]) -> Option<String> {
    match serde_json::to_string_pretty(items) {
        Ok(data) => Some(data),
        Err(error) => {
            eprintln!("[FamVoice] Failed to serialize history: {}", error);
            None
        }
    }
}

fn truncate_history_text(text: String) -> String {
    if text.chars().count() <= MAX_HISTORY_ITEM_CHARS {
        return text;
    }

    text.chars().take(MAX_HISTORY_ITEM_CHARS).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_history_add_delete_clear() {
        let dir = tempdir().unwrap();
        let state = HistoryState::load(dir.path().to_path_buf());

        state.add("Item 1".to_string());
        std::thread::sleep(std::time::Duration::from_millis(10));
        state.add("Item 2".to_string());

        {
            let items = state.items.lock().unwrap();
            assert_eq!(items.len(), 2);
            assert_eq!(items[0].text, "Item 2"); // Newest first
            assert_eq!(items[1].text, "Item 1");
        }

        let id_to_delete = {
            let items = state.items.lock().unwrap();
            items[1].id
        };

        state.delete(id_to_delete);

        {
            let items = state.items.lock().unwrap();
            assert_eq!(items.len(), 1);
            assert_eq!(items[0].text, "Item 2");
        }

        state.clear();

        {
            let items = state.items.lock().unwrap();
            assert_eq!(items.len(), 0);
        }
    }

    #[test]
    fn test_history_add_truncates_large_items() {
        let dir = tempdir().unwrap();
        let state = HistoryState::load(dir.path().to_path_buf());
        let text = "a".repeat(MAX_HISTORY_ITEM_CHARS + 25);

        state.add(text);

        let items = state.items.lock().unwrap();
        assert_eq!(items[0].text.len(), MAX_HISTORY_ITEM_CHARS);
    }
}
