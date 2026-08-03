use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_HISTORY_ITEM_CHARS: usize = 50_000;
pub const DEFAULT_HISTORY_MAX_ITEMS: usize = 100;
pub const MAX_HISTORY_MAX_ITEMS: usize = 100;
const HISTORY_FILE_VERSION: u8 = 2;
const LEGACY_HISTORY_FILE_VERSION: u8 = 1;
const HISTORY_DISK_CONTEXT: &str = "transcript history";
const PURGE_MARKER_SUFFIX: &str = ".purge";

#[derive(Serialize, Deserialize)]
struct HistoryDiskEnvelope {
    version: u8,
    payload: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryRetentionPolicy {
    #[serde(alias = "max_items")]
    pub max_items: usize,
}

impl Default for HistoryRetentionPolicy {
    fn default() -> Self {
        Self {
            max_items: DEFAULT_HISTORY_MAX_ITEMS,
        }
    }
}

#[derive(Default, Serialize, Deserialize)]
struct HistoryDiskState {
    #[serde(default)]
    items: Vec<HistoryItem>,
    #[serde(default)]
    retention: HistoryRetentionPolicy,
    /// A durable deletion intent. If a crash leaves this marker in either the
    /// main file or its sidecar, the next load completes the purge instead of
    /// recovering an older backup.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    purge_generation: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryItem {
    pub id: u64,
    pub text: String,
    pub timestamp: u64,
    #[serde(default)]
    pub pinned: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HistoryExportFormat {
    Txt,
    Markdown,
    Json,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreparedHistoryExport {
    pub suggested_file_name: String,
    pub media_type: String,
    pub contents: String,
}

pub struct HistoryState {
    pub items: Mutex<Vec<HistoryItem>>,
    storage: crate::persistence::AtomicFile,
    next_id: AtomicU64,
    max_items: AtomicUsize,
}

impl HistoryState {
    pub fn load(app_dir: PathBuf) -> Self {
        let path = app_dir.join("history.json");
        let purge_path = sibling_path(&path, PURGE_MARKER_SUFFIX);
        let storage = crate::persistence::AtomicFile::new(path.clone());

        if purge_path.exists() {
            let policy = retention_from_pending_purge(&path, &purge_path);
            let state = Self::empty(storage, policy.clone());
            if let Err(error) = state.finish_pending_purge(&purge_path, policy, None) {
                eprintln!("[FamVoice] Failed to complete pending history purge: {error}");
            }
            return state;
        }

        let mut recovery_data = None;
        let (disk_state, mut needs_resave) = if path.exists() {
            match fs::read_to_string(&path) {
                Ok(data) => parse_history_state(&data).unwrap_or_else(|e| {
                    eprintln!(
                        "[FamVoice] Failed to parse history.json: {}, preserving corrupt file",
                        e
                    );
                    let _ = crate::persistence::preserve_corrupt_file(&path);
                    recover_history_backup(&storage, &mut recovery_data)
                }),
                Err(e) => {
                    eprintln!(
                        "[FamVoice] Failed to read history.json: {}, preserving corrupt file",
                        e
                    );
                    let _ = crate::persistence::preserve_corrupt_file(&path);
                    recover_history_backup(&storage, &mut recovery_data)
                }
            }
        } else {
            (HistoryDiskState::default(), false)
        };

        if disk_state.purge_generation.is_some() {
            let policy = disk_state.retention;
            let state = Self::empty(storage, policy.clone());
            if let Err(error) = state.finish_pending_purge(&purge_path, policy, None) {
                eprintln!("[FamVoice] Failed to complete interrupted history purge: {error}");
            }
            return state;
        }

        let HistoryDiskState {
            items,
            mut retention,
            purge_generation: _,
        } = disk_state;
        if retention.max_items > MAX_HISTORY_MAX_ITEMS {
            retention.max_items = MAX_HISTORY_MAX_ITEMS;
            needs_resave = true;
        }
        let next_id = items
            .iter()
            .map(|item| item.id)
            .max()
            .unwrap_or(0)
            .saturating_add(1);

        let state = Self {
            items: Mutex::new(items),
            storage,
            next_id: AtomicU64::new(next_id),
            max_items: AtomicUsize::new(retention.max_items),
        };

        if let Some(data) = recovery_data {
            if let Err(error) = state.storage.restore_known_good(data.as_bytes()) {
                eprintln!("[FamVoice] Failed to restore recovered history: {error}");
            }
        } else if needs_resave {
            match state.serialize_current() {
                Ok(data) => {
                    // Do not preserve a legacy plaintext or v1 file as the
                    // recovery copy after migration to the encrypted v2 state.
                    if let Err(error) = state.storage.restore_known_good(data.as_bytes()) {
                        eprintln!("[FamVoice] Failed to migrate history state: {error}");
                    }
                }
                Err(error) => eprintln!("[FamVoice] Failed to serialize history: {error}"),
            }
        }

        state
    }

    fn empty(storage: crate::persistence::AtomicFile, retention: HistoryRetentionPolicy) -> Self {
        Self {
            items: Mutex::new(Vec::new()),
            storage,
            next_id: AtomicU64::new(1),
            max_items: AtomicUsize::new(retention.max_items),
        }
    }

    pub fn retention_policy(&self) -> HistoryRetentionPolicy {
        HistoryRetentionPolicy {
            max_items: self.max_items.load(Ordering::Acquire),
        }
    }

    /// Persists a new retention limit without deleting existing entries.
    /// `max_items == 0` disables future `add` operations; existing entries stay
    /// available until the user explicitly deletes or purges them.
    pub fn set_retention_policy(&self, policy: HistoryRetentionPolicy) -> Result<(), String> {
        if policy.max_items > MAX_HISTORY_MAX_ITEMS {
            return Err(format!(
                "History retention cannot exceed {MAX_HISTORY_MAX_ITEMS} items"
            ));
        }
        let (serialized, revision) = {
            let items = self.items.lock().unwrap_or_else(|e| {
                eprintln!("[FamVoice] History lock poisoned in set_retention_policy(), recovering");
                e.into_inner()
            });
            self.max_items.store(policy.max_items, Ordering::Release);
            (
                encode_history_state(&HistoryDiskState {
                    items: items.clone(),
                    retention: policy,
                    purge_generation: None,
                })?,
                self.storage.reserve_revision(),
            )
        };
        self.write_to_disk(revision, serialized)
    }

    pub fn set_max_items(&self, max_items: usize) -> Result<(), String> {
        self.set_retention_policy(HistoryRetentionPolicy { max_items })
    }

    pub fn add(&self, text: String) -> Result<(), String> {
        let (serialized, revision) = {
            let mut items = self.items.lock().unwrap_or_else(|e| {
                eprintln!("[FamVoice] History lock poisoned in add(), recovering");
                e.into_inner()
            });
            let max_items = self.max_items.load(Ordering::Acquire);
            if max_items == 0 {
                return Ok(());
            }

            let timestamp = unix_timestamp_millis();
            let id = self.next_id.fetch_add(1, Ordering::Relaxed);
            items.insert(
                0,
                HistoryItem {
                    id,
                    text: truncate_history_text(text),
                    timestamp,
                    pinned: false,
                },
            );
            if items.len() > max_items {
                items.truncate(max_items);
            }
            (
                self.encode_snapshot(&items)?,
                self.storage.reserve_revision(),
            )
        };
        self.write_to_disk(revision, serialized)
    }

    pub fn delete(&self, id: u64) -> Result<HistoryItem, String> {
        let (deleted_item, serialized, revision) = {
            let mut items = self.items.lock().unwrap_or_else(|e| {
                eprintln!("[FamVoice] History lock poisoned in delete(), recovering");
                e.into_inner()
            });
            let item_index = items
                .iter()
                .position(|item| item.id == id)
                .ok_or_else(|| "History item no longer exists".to_string())?;
            let deleted_item = items.remove(item_index);
            (
                deleted_item,
                self.encode_snapshot(&items)?,
                self.storage.reserve_revision(),
            )
        };
        self.write_to_disk(revision, serialized)?;
        Ok(deleted_item)
    }

    pub fn restore(&self, mut item: HistoryItem) -> Result<(), String> {
        let (serialized, revision) = {
            let mut items = self.items.lock().unwrap_or_else(|e| {
                eprintln!("[FamVoice] History lock poisoned in restore(), recovering");
                e.into_inner()
            });

            if items.iter().any(|existing| existing.id == item.id) {
                return Ok(());
            }

            item.text = truncate_history_text(item.text);
            self.next_id
                .fetch_max(item.id.saturating_add(1), Ordering::Relaxed);
            items.push(item);
            items.sort_by(canonical_history_order);

            // Undo is explicit and must not silently evict another item, even
            // if the retention preference was reduced after the deletion.
            (
                self.encode_snapshot(&items)?,
                self.storage.reserve_revision(),
            )
        };
        self.write_to_disk(revision, serialized)
    }

    /// Toggles a pin without sorting or otherwise changing canonical history
    /// order. The returned value is the item's new pin state.
    pub fn toggle_pin(&self, id: u64) -> Result<bool, String> {
        let (pinned, serialized, revision) = {
            let mut items = self.items.lock().unwrap_or_else(|e| {
                eprintln!("[FamVoice] History lock poisoned in toggle_pin(), recovering");
                e.into_inner()
            });
            let item = items
                .iter_mut()
                .find(|item| item.id == id)
                .ok_or_else(|| "History item no longer exists".to_string())?;
            item.pinned = !item.pinned;
            let pinned = item.pinned;
            (
                pinned,
                self.encode_snapshot(&items)?,
                self.storage.reserve_revision(),
            )
        };
        self.write_to_disk(revision, serialized)?;
        Ok(pinned)
    }

    /// Permanently clears transcript history. A durable purge marker is written
    /// first; backups, corrupt copies and atomic-write temporaries are removed
    /// before the marker is retired. `load` resumes an interrupted purge.
    pub fn clear(&self) -> Result<(), String> {
        let mut items = self.items.lock().unwrap_or_else(|e| {
            eprintln!("[FamVoice] History lock poisoned in clear(), recovering");
            e.into_inner()
        });
        let policy = self.retention_policy();
        let purge_path = sibling_path(self.storage.path(), PURGE_MARKER_SUFFIX);
        let purge_state = HistoryDiskState {
            items: Vec::new(),
            retention: policy.clone(),
            purge_generation: Some(unix_timestamp_millis()),
        };
        let serialized = encode_history_state(&purge_state)?;

        write_purge_marker(&purge_path, serialized.as_bytes())?;
        items.clear();
        let revision = self.storage.reserve_revision();
        self.finish_pending_purge(&purge_path, policy, Some((revision, serialized)))
    }

    pub fn prepare_export(
        &self,
        format: HistoryExportFormat,
    ) -> Result<PreparedHistoryExport, String> {
        let items = self
            .items
            .lock()
            .map_err(|_| "Failed to lock history for export".to_string())?
            .clone();
        prepare_history_export(&items, format)
    }

    fn encode_snapshot(&self, items: &[HistoryItem]) -> Result<String, String> {
        encode_history_state(&HistoryDiskState {
            items: items.to_vec(),
            retention: self.retention_policy(),
            purge_generation: None,
        })
    }

    fn serialize_current(&self) -> Result<String, String> {
        let items = self
            .items
            .lock()
            .map_err(|_| "Failed to lock history for serialization".to_string())?;
        self.encode_snapshot(&items)
    }

    fn finish_pending_purge(
        &self,
        purge_path: &Path,
        policy: HistoryRetentionPolicy,
        reserved_write: Option<(u64, String)>,
    ) -> Result<(), String> {
        let purge_state = HistoryDiskState {
            items: Vec::new(),
            retention: policy.clone(),
            purge_generation: Some(unix_timestamp_millis()),
        };
        let purge_serialized = match reserved_write.as_ref() {
            Some((_, serialized)) => serialized.clone(),
            None => encode_history_state(&purge_state)?,
        };

        if !purge_path.exists() {
            write_purge_marker(purge_path, purge_serialized.as_bytes())?;
        }

        match reserved_write {
            Some((revision, serialized)) => {
                self.storage.write(revision, serialized.as_bytes())?;
            }
            None => self
                .storage
                .restore_known_good(purge_serialized.as_bytes())?,
        }

        remove_history_recovery_artifacts(self.storage.path(), purge_path)?;

        let clean_state = HistoryDiskState {
            items: Vec::new(),
            retention: policy,
            purge_generation: None,
        };
        let clean_serialized = encode_history_state(&clean_state)?;
        self.storage
            .restore_known_good(clean_serialized.as_bytes())?;

        remove_file_if_present(purge_path)?;
        // Catch a temporary left by a failed cleanup/write attempt before the
        // purge marker was retired.
        remove_history_recovery_artifacts(self.storage.path(), purge_path)
    }

    /// File I/O happens after the items lock is released. AtomicFile serializes
    /// disk access and rejects late snapshots using the reserved revision.
    fn write_to_disk(&self, revision: u64, serialized: String) -> Result<(), String> {
        self.storage
            .write(revision, serialized.as_bytes())
            .map(|_| ())
    }
}

fn recover_history_backup(
    storage: &crate::persistence::AtomicFile,
    recovery_data: &mut Option<String>,
) -> (HistoryDiskState, bool) {
    let Ok(data) = fs::read_to_string(storage.backup_path()) else {
        return (HistoryDiskState::default(), false);
    };

    match parse_history_state(&data) {
        Ok((disk_state, needs_resave)) => {
            *recovery_data = if needs_resave {
                encode_history_state(&disk_state).ok()
            } else {
                Some(data)
            };
            (disk_state, false)
        }
        Err(error) => {
            eprintln!("[FamVoice] History recovery copy is invalid: {error}");
            (HistoryDiskState::default(), false)
        }
    }
}

fn parse_history_state(data: &str) -> Result<(HistoryDiskState, bool), String> {
    if let Ok(items) = serde_json::from_str::<Vec<HistoryItem>>(data) {
        return Ok((legacy_state(items), true));
    }

    if let Ok(envelope) = serde_json::from_str::<HistoryDiskEnvelope>(data) {
        let decrypted_json = decrypt_history_payload(&envelope.payload)?;
        return match envelope.version {
            LEGACY_HISTORY_FILE_VERSION => {
                let items = serde_json::from_str::<Vec<HistoryItem>>(&decrypted_json)
                    .map_err(|error| format!("invalid legacy history payload: {error}"))?;
                Ok((legacy_state(items), true))
            }
            HISTORY_FILE_VERSION => {
                let state = serde_json::from_str::<HistoryDiskState>(&decrypted_json)
                    .map_err(|error| format!("invalid history payload: {error}"))?;
                Ok((state, false))
            }
            version => Err(format!("unsupported history file version {version}")),
        };
    }

    serde_json::from_str::<HistoryDiskState>(data)
        .map(|state| (state, true))
        .map_err(|error| format!("unknown history format: {error}"))
}

fn legacy_state(items: Vec<HistoryItem>) -> HistoryDiskState {
    HistoryDiskState {
        items,
        retention: HistoryRetentionPolicy::default(),
        purge_generation: None,
    }
}

fn decrypt_history_payload(payload: &str) -> Result<String, String> {
    #[cfg(windows)]
    {
        crate::dpapi::unprotect_string(payload, HISTORY_DISK_CONTEXT)
    }

    #[cfg(not(windows))]
    {
        Ok(payload.to_string())
    }
}

fn encode_history_state(state: &HistoryDiskState) -> Result<String, String> {
    let plaintext_json = serde_json::to_string_pretty(state)
        .map_err(|error| format!("failed to serialize history state: {error}"))?;

    #[cfg(windows)]
    let payload = crate::dpapi::protect_string(&plaintext_json, HISTORY_DISK_CONTEXT)?;

    #[cfg(not(windows))]
    let payload = plaintext_json;

    serde_json::to_string_pretty(&HistoryDiskEnvelope {
        version: HISTORY_FILE_VERSION,
        payload,
    })
    .map_err(|error| format!("failed to serialize encrypted history envelope: {error}"))
}

fn retention_from_pending_purge(path: &Path, purge_path: &Path) -> HistoryRetentionPolicy {
    let mut policy = [purge_path, path]
        .into_iter()
        .find_map(|candidate| {
            fs::read_to_string(candidate)
                .ok()
                .and_then(|data| parse_history_state(&data).ok())
                .map(|(state, _)| state.retention)
        })
        .unwrap_or_default();
    policy.max_items = policy.max_items.min(MAX_HISTORY_MAX_ITEMS);
    policy
}

fn write_purge_marker(path: &Path, data: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Failed to prepare history purge: {error}"))?;
    }

    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .map_err(|error| format!("Failed to start history purge: {error}"))?;
    file.write_all(data)
        .and_then(|_| file.flush())
        .and_then(|_| file.sync_all())
        .map_err(|error| format!("Failed to persist history purge marker: {error}"))
}

fn remove_history_recovery_artifacts(path: &Path, purge_path: &Path) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "History file has no parent directory".to_string())?;
    let target_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "History file name is invalid".to_string())?;
    let backup_name = format!("{target_name}.bak");
    let corrupt_name = format!("{target_name}.corrupt");
    let temporary_prefix = format!(".{target_name}.");

    let entries = fs::read_dir(parent)
        .map_err(|error| format!("Failed to inspect history recovery files: {error}"))?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("Failed to inspect history file: {error}"))?;
        let candidate = entry.path();
        if candidate == path || candidate == purge_path {
            continue;
        }
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let is_backup = name == backup_name;
        let is_corrupt = name == corrupt_name || name.starts_with(&format!("{corrupt_name}."));
        let is_temporary = name.starts_with(&temporary_prefix) && name.ends_with(".tmp");
        if is_backup || is_corrupt || is_temporary {
            remove_file_if_present(&candidate)?;
        }
    }
    Ok(())
}

fn remove_file_if_present(path: &Path) -> Result<(), String> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("Failed to remove {}: {error}", path.display())),
    }
}

fn sibling_path(path: &Path, suffix: &str) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("history.json");
    path.with_file_name(format!("{file_name}{suffix}"))
}

fn unix_timestamp_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn canonical_history_order(left: &HistoryItem, right: &HistoryItem) -> std::cmp::Ordering {
    right
        .timestamp
        .cmp(&left.timestamp)
        .then_with(|| right.id.cmp(&left.id))
}

fn prepare_history_export(
    items: &[HistoryItem],
    format: HistoryExportFormat,
) -> Result<PreparedHistoryExport, String> {
    match format {
        HistoryExportFormat::Txt => Ok(PreparedHistoryExport {
            suggested_file_name: "famvoice-history.txt".to_string(),
            media_type: "text/plain;charset=utf-8".to_string(),
            contents: items
                .iter()
                .map(|item| item.text.as_str())
                .collect::<Vec<_>>()
                .join("\n\n---\n\n"),
        }),
        HistoryExportFormat::Markdown => {
            let mut contents = String::from("# FamVoice history\n");
            for item in items {
                contents.push_str(&format!(
                    "\n## Transcript {}{}\n\n- Timestamp (Unix ms): {}\n- Pinned: {}\n\n",
                    item.id,
                    if item.pinned { " (pinned)" } else { "" },
                    item.timestamp,
                    if item.pinned { "yes" } else { "no" },
                ));
                for line in item.text.lines() {
                    contents.push_str("> ");
                    contents.push_str(line);
                    contents.push('\n');
                }
                if item.text.is_empty() {
                    contents.push_str(">\n");
                }
            }
            Ok(PreparedHistoryExport {
                suggested_file_name: "famvoice-history.md".to_string(),
                media_type: "text/markdown;charset=utf-8".to_string(),
                contents,
            })
        }
        HistoryExportFormat::Json => Ok(PreparedHistoryExport {
            suggested_file_name: "famvoice-history.json".to_string(),
            media_type: "application/json".to_string(),
            contents: serde_json::to_string_pretty(items)
                .map_err(|error| format!("Failed to prepare history JSON export: {error}"))?,
        }),
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

    fn item(id: u64, text: &str, timestamp: u64, pinned: bool) -> HistoryItem {
        HistoryItem {
            id,
            text: text.to_string(),
            timestamp,
            pinned,
        }
    }

    #[test]
    fn test_history_add_delete_clear() {
        let dir = tempdir().unwrap();
        let state = HistoryState::load(dir.path().to_path_buf());

        state.add("Item 1".to_string()).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        state.add("Item 2".to_string()).unwrap();

        {
            let items = state.items.lock().unwrap();
            assert_eq!(items.len(), 2);
            assert_eq!(items[0].text, "Item 2");
            assert_eq!(items[1].text, "Item 1");
        }

        let id_to_delete = state.items.lock().unwrap()[1].id;
        state.delete(id_to_delete).unwrap();
        assert_eq!(state.items.lock().unwrap().len(), 1);

        state.clear().unwrap();
        assert!(state.items.lock().unwrap().is_empty());
    }

    #[test]
    fn test_history_delete_can_be_undone_with_original_identity_order_and_pin() {
        let dir = tempdir().unwrap();
        let state = HistoryState::load(dir.path().to_path_buf());

        state.add("Older item".to_string()).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        state.add("Newer item".to_string()).unwrap();

        let older_id = state.items.lock().unwrap()[1].id;
        state.toggle_pin(older_id).unwrap();
        let deleted_item = state.delete(older_id).unwrap();
        assert!(deleted_item.pinned);
        state.restore(deleted_item.clone()).unwrap();

        let restored_items = state.items.lock().unwrap().clone();
        assert_eq!(restored_items.len(), 2);
        assert_eq!(restored_items[0].text, "Newer item");
        assert_eq!(restored_items[1], deleted_item);
        drop(restored_items);

        let reloaded = HistoryState::load(dir.path().to_path_buf());
        assert_eq!(reloaded.items.lock().unwrap()[1], deleted_item);
    }

    #[test]
    fn test_history_add_truncates_large_items() {
        let dir = tempdir().unwrap();
        let state = HistoryState::load(dir.path().to_path_buf());
        let text = "a".repeat(MAX_HISTORY_ITEM_CHARS + 25);

        state.add(text).unwrap();

        let items = state.items.lock().unwrap();
        assert_eq!(items[0].text.len(), MAX_HISTORY_ITEM_CHARS);
    }

    #[test]
    fn test_history_preserves_text_refused_by_delivery_limit() {
        let dir = tempdir().unwrap();
        let state = HistoryState::load(dir.path().to_path_buf());
        let text = "🦀".repeat(crate::delivery::MAX_DELIVERED_TEXT_CHARS + 1);

        assert!(crate::delivery::validate_text_length(&text).is_err());
        state.add(text.clone()).unwrap();

        assert_eq!(state.items.lock().unwrap()[0].text, text);
    }

    #[test]
    fn test_history_reloads_from_disk_after_write() {
        let dir = tempdir().unwrap();
        let state = HistoryState::load(dir.path().to_path_buf());
        state.add("Persisted item".to_string()).unwrap();

        let reloaded = HistoryState::load(dir.path().to_path_buf());
        assert_eq!(reloaded.items.lock().unwrap()[0].text, "Persisted item");
    }

    #[test]
    fn test_plaintext_history_migrates_pinned_default_and_default_retention() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("history.json");
        fs::write(
            &path,
            r#"[
  { "id": 1, "text": "Legacy item", "timestamp": 123 }
]"#,
        )
        .unwrap();

        let state = HistoryState::load(dir.path().to_path_buf());
        assert!(!state.items.lock().unwrap()[0].pinned);
        assert_eq!(
            state.retention_policy().max_items,
            DEFAULT_HISTORY_MAX_ITEMS
        );

        let migrated = fs::read_to_string(path).unwrap();
        let envelope: HistoryDiskEnvelope = serde_json::from_str(&migrated).unwrap();
        assert_eq!(envelope.version, HISTORY_FILE_VERSION);
        assert!(!migrated.contains("Legacy item"));
        assert!(!dir.path().join("history.json.bak").exists());
    }

    #[test]
    fn test_v1_encrypted_envelope_migrates_policy_without_increasing_default() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("history.json");
        let legacy_json = r#"[{"id":7,"text":"Encrypted legacy","timestamp":456}]"#;
        #[cfg(windows)]
        let payload = crate::dpapi::protect_string(legacy_json, HISTORY_DISK_CONTEXT).unwrap();
        #[cfg(not(windows))]
        let payload = legacy_json.to_string();
        let envelope = serde_json::to_string_pretty(&HistoryDiskEnvelope {
            version: LEGACY_HISTORY_FILE_VERSION,
            payload,
        })
        .unwrap();
        fs::write(&path, envelope).unwrap();

        let state = HistoryState::load(dir.path().to_path_buf());
        assert_eq!(state.retention_policy().max_items, 100);
        assert!(!state.items.lock().unwrap()[0].pinned);

        let migrated: HistoryDiskEnvelope =
            serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap();
        assert_eq!(migrated.version, HISTORY_FILE_VERSION);
    }

    #[test]
    fn test_default_retention_remains_bounded_at_one_hundred() {
        let dir = tempdir().unwrap();
        let state = HistoryState::load(dir.path().to_path_buf());

        for index in 0..=DEFAULT_HISTORY_MAX_ITEMS {
            state.add(format!("Item {index}")).unwrap();
        }

        let items = state.items.lock().unwrap();
        assert_eq!(items.len(), 100);
        assert_eq!(items[0].text, "Item 100");
        assert_eq!(items.last().unwrap().text, "Item 1");
    }

    #[test]
    fn retention_cannot_be_increased_above_the_documented_limit() {
        let dir = tempdir().unwrap();
        let state = HistoryState::load(dir.path().to_path_buf());

        assert!(state.set_max_items(MAX_HISTORY_MAX_ITEMS + 1).is_err());
        assert_eq!(
            state.retention_policy().max_items,
            DEFAULT_HISTORY_MAX_ITEMS
        );
    }

    #[test]
    fn test_retention_is_persisted_but_reducing_it_does_not_delete_existing_items() {
        let dir = tempdir().unwrap();
        let state = HistoryState::load(dir.path().to_path_buf());
        for index in 0..3 {
            state.add(format!("Existing {index}")).unwrap();
        }

        state.set_max_items(2).unwrap();
        assert_eq!(state.items.lock().unwrap().len(), 3);

        let reloaded = HistoryState::load(dir.path().to_path_buf());
        assert_eq!(reloaded.retention_policy().max_items, 2);
        assert_eq!(reloaded.items.lock().unwrap().len(), 3);

        reloaded.add("New recording".to_string()).unwrap();
        let items = reloaded.items.lock().unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].text, "New recording");
    }

    #[test]
    fn test_zero_retention_disables_new_recordings_without_deleting_existing_items() {
        let dir = tempdir().unwrap();
        let state = HistoryState::load(dir.path().to_path_buf());
        state.add("Keep me".to_string()).unwrap();
        state.set_max_items(0).unwrap();
        state.add("Do not record me".to_string()).unwrap();

        assert_eq!(state.items.lock().unwrap().len(), 1);
        assert_eq!(state.items.lock().unwrap()[0].text, "Keep me");

        let reloaded = HistoryState::load(dir.path().to_path_buf());
        assert_eq!(reloaded.retention_policy().max_items, 0);
        assert_eq!(reloaded.items.lock().unwrap()[0].text, "Keep me");
    }

    #[test]
    fn test_toggle_pin_persists_without_changing_canonical_order() {
        let dir = tempdir().unwrap();
        let state = HistoryState::load(dir.path().to_path_buf());
        state.add("Older".to_string()).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        state.add("Newer".to_string()).unwrap();

        let ids_before = state
            .items
            .lock()
            .unwrap()
            .iter()
            .map(|item| item.id)
            .collect::<Vec<_>>();
        assert!(state.toggle_pin(ids_before[1]).unwrap());
        let items = state.items.lock().unwrap();
        assert_eq!(
            items.iter().map(|item| item.id).collect::<Vec<_>>(),
            ids_before
        );
        assert!(items[1].pinned);
        drop(items);

        let reloaded = HistoryState::load(dir.path().to_path_buf());
        let items = reloaded.items.lock().unwrap();
        assert_eq!(
            items.iter().map(|item| item.id).collect::<Vec<_>>(),
            ids_before
        );
        assert!(items[1].pinned);
    }

    #[test]
    fn test_exports_are_explicit_deterministic_and_include_pin_metadata() {
        let items = vec![
            item(2, "Olá\nmundo", 200, true),
            item(1, "Segundo", 100, false),
        ];

        let txt = prepare_history_export(&items, HistoryExportFormat::Txt).unwrap();
        assert_eq!(txt.suggested_file_name, "famvoice-history.txt");
        assert_eq!(txt.contents, "Olá\nmundo\n\n---\n\nSegundo");

        let markdown = prepare_history_export(&items, HistoryExportFormat::Markdown).unwrap();
        assert!(markdown.contents.contains("## Transcript 2 (pinned)"));
        assert!(markdown.contents.contains("> Olá\n> mundo"));

        let json = prepare_history_export(&items, HistoryExportFormat::Json).unwrap();
        let decoded: Vec<HistoryItem> = serde_json::from_str(&json.contents).unwrap();
        assert_eq!(decoded, items);
        assert!(json.contents.contains("\"pinned\": true"));
    }

    #[test]
    fn test_purge_removes_backup_corrupt_and_temporary_recovery_paths() {
        let dir = tempdir().unwrap();
        let state = HistoryState::load(dir.path().to_path_buf());
        state.add("First secret".to_string()).unwrap();
        state.add("Second secret".to_string()).unwrap();
        state.set_max_items(12).unwrap();

        fs::write(dir.path().join("history.json.corrupt"), "First secret").unwrap();
        fs::write(dir.path().join("history.json.corrupt.77"), "Second secret").unwrap();
        fs::write(
            dir.path().join(".history.json.123.pending.9.tmp"),
            "temporary secret",
        )
        .unwrap();

        state.clear().unwrap();

        let remaining_names = fs::read_dir(dir.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert_eq!(remaining_names, vec!["history.json"]);
        let reloaded = HistoryState::load(dir.path().to_path_buf());
        assert!(reloaded.items.lock().unwrap().is_empty());
        assert_eq!(reloaded.retention_policy().max_items, 12);
    }

    #[test]
    fn test_load_resumes_crash_interrupted_purge_instead_of_recovering_backup() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("history.json");
        let state = HistoryState::load(dir.path().to_path_buf());
        state.add("Must not recover".to_string()).unwrap();
        state.add("Also private".to_string()).unwrap();

        let purge_state = HistoryDiskState {
            items: Vec::new(),
            retention: HistoryRetentionPolicy { max_items: 12 },
            purge_generation: Some(42),
        };
        let purge_serialized = encode_history_state(&purge_state).unwrap();
        write_purge_marker(
            &sibling_path(&path, PURGE_MARKER_SUFFIX),
            purge_serialized.as_bytes(),
        )
        .unwrap();

        let recovered = HistoryState::load(dir.path().to_path_buf());
        assert!(recovered.items.lock().unwrap().is_empty());
        assert_eq!(recovered.retention_policy().max_items, 12);
        assert!(!dir.path().join("history.json.bak").exists());
        assert!(!dir.path().join("history.json.purge").exists());
    }

    #[test]
    fn test_history_loads_corrupted_file_as_empty_and_creates_backup() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("history.json");
        let corrupted_contents = "this is not valid json";
        fs::write(&path, corrupted_contents).unwrap();

        let state = HistoryState::load(dir.path().to_path_buf());
        assert!(state.items.lock().unwrap().is_empty());

        let backup_path = dir.path().join("history.json.corrupt");
        assert!(backup_path.exists());
        assert_eq!(fs::read_to_string(backup_path).unwrap(), corrupted_contents);
    }

    #[test]
    fn test_history_recovers_last_known_good_backup() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("history.json");
        let state = HistoryState::load(dir.path().to_path_buf());
        state.add("First durable item".to_string()).unwrap();
        state.add("Second durable item".to_string()).unwrap();
        fs::write(&path, "interrupted-json").unwrap();

        let recovered = HistoryState::load(dir.path().to_path_buf());
        let items = recovered.items.lock().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].text, "First durable item");
        drop(items);
        assert!(parse_history_state(&fs::read_to_string(path).unwrap()).is_ok());
    }

    #[test]
    fn test_concurrent_history_operations_persist_the_latest_snapshot() {
        let dir = tempdir().unwrap();
        let state = std::sync::Arc::new(HistoryState::load(dir.path().to_path_buf()));
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(9));
        let mut writers = Vec::new();

        for index in 0..8 {
            let state = std::sync::Arc::clone(&state);
            let barrier = std::sync::Arc::clone(&barrier);
            writers.push(std::thread::spawn(move || {
                barrier.wait();
                state.add(format!("Concurrent item {index}"))
            }));
        }
        barrier.wait();
        for writer in writers {
            writer.join().unwrap().unwrap();
        }

        let memory_items = state.items.lock().unwrap().clone();
        let reloaded = HistoryState::load(dir.path().to_path_buf());
        let disk_items = reloaded.items.lock().unwrap().clone();
        assert_eq!(disk_items.len(), 8);
        assert_eq!(
            disk_items.iter().map(|item| item.id).collect::<Vec<_>>(),
            memory_items.iter().map(|item| item.id).collect::<Vec<_>>()
        );
    }
}
