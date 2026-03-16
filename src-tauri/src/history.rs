use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Serialize, Deserialize)]
pub struct HistoryItem {
    pub id: u64,
    pub text: String,
    pub timestamp: u64,
}

pub struct HistoryState {
    pub items: Mutex<Vec<HistoryItem>>,
    pub path: PathBuf,
}

impl HistoryState {
    pub fn load(app_dir: PathBuf) -> Self {
        let path = app_dir.join("history.json");
        let items = if path.exists() {
            let data = fs::read_to_string(&path).unwrap_or_default();
            serde_json::from_str(&data).unwrap_or_default()
        } else {
            Vec::new()
        };
        Self {
            items: Mutex::new(items),
            path,
        }
    }

    pub fn add(&self, text: String) {
        let mut items = self.items.lock().unwrap();
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64;
        items.insert(0, HistoryItem {
            id: timestamp,
            text,
            timestamp,
        });
        if items.len() > 100 {
            items.truncate(100);
        }
        self.save_locked(&items);
    }

    pub fn delete(&self, id: u64) {
        let mut items = self.items.lock().unwrap();
        items.retain(|item| item.id != id);
        self.save_locked(&items);
    }

    pub fn clear(&self) {
        let mut items = self.items.lock().unwrap();
        items.clear();
        self.save_locked(&items);
    }

    fn save_locked(&self, items: &[HistoryItem]) {
        if let Ok(data) = serde_json::to_string_pretty(items) {
            let _ = fs::write(&self.path, data);
        }
    }
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
}