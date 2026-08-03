use arboard::Clipboard;
use std::future::Future;
use std::sync::Mutex;
use std::time::Duration;
use tokio::sync::{Mutex as AsyncMutex, MutexGuard as AsyncMutexGuard};

pub struct ClipboardState {
    clipboard: Mutex<Option<Clipboard>>,
    transaction: AsyncMutex<()>,
}

impl Default for ClipboardState {
    fn default() -> Self {
        Self {
            clipboard: Mutex::new(None),
            transaction: AsyncMutex::new(()),
        }
    }
}

impl ClipboardState {
    pub async fn lock_transaction(&self) -> AsyncMutexGuard<'_, ()> {
        self.transaction.lock().await
    }

    pub fn transaction_lock(&self) -> &AsyncMutex<()> {
        &self.transaction
    }
}

fn with_clipboard<T>(
    state: &ClipboardState,
    action: impl FnOnce(&mut Clipboard) -> Result<T, String>,
) -> Result<T, String> {
    let mut clipboard_guard = state
        .clipboard
        .lock()
        .map_err(|_| "Clipboard mutex poisoned".to_string())?;

    if clipboard_guard.is_none() {
        *clipboard_guard = Some(Clipboard::new().map_err(|e| e.to_string())?);
    }

    let clipboard = clipboard_guard
        .as_mut()
        .expect("clipboard should be initialized");
    action(clipboard)
}

pub fn read_clipboard_text(state: &ClipboardState) -> Result<String, String> {
    with_clipboard(state, |clipboard| {
        clipboard.get_text().map_err(|e| e.to_string())
    })
}

pub fn restore_clipboard_text(state: &ClipboardState, text: &str) -> Result<(), String> {
    with_clipboard(state, |clipboard| {
        clipboard
            .set_text(text.to_string())
            .map_err(|e| e.to_string())
    })
}

pub fn set_clipboard(state: &ClipboardState, text: &str) -> Result<(), String> {
    restore_clipboard_text(state, text)
}

pub async fn run_temporary_text_transaction<Read, Write, Paste, PasteFuture>(
    transaction: &AsyncMutex<()>,
    text: &str,
    settle_delay: Duration,
    restore_delay: Duration,
    read: Read,
    mut write: Write,
    paste: Paste,
) -> Result<(), String>
where
    Read: FnOnce() -> Result<String, String>,
    Write: FnMut(&str) -> Result<(), String>,
    Paste: FnOnce() -> PasteFuture,
    PasteFuture: Future<Output = Result<(), String>>,
{
    let _transaction_guard = transaction.lock().await;
    let original_text = read().map_err(|error| format!("Failed to read clipboard: {error}"))?;

    write(text).map_err(|error| format!("Failed to set clipboard: {error}"))?;
    tokio::time::sleep(settle_delay).await;
    let paste_result = paste().await;

    tokio::time::sleep(restore_delay).await;
    let restore_result =
        write(&original_text).map_err(|error| format!("Failed to restore clipboard: {error}"));

    match (paste_result, restore_result) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(paste_error), Ok(())) => Err(paste_error),
        (Ok(()), Err(restore_error)) => Err(restore_error),
        (Err(paste_error), Err(restore_error)) => Err(format!("{paste_error}; {restore_error}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    async fn assert_overlapping_transactions_preserve_snapshot(texts: &[&'static str]) {
        let transaction = Arc::new(AsyncMutex::new(()));
        let clipboard = Arc::new(Mutex::new("original Ω\nline two".to_string()));
        let pasted = Arc::new(Mutex::new(Vec::new()));
        let mut handles = Vec::new();

        for text in texts.iter().copied() {
            let transaction = Arc::clone(&transaction);
            let clipboard_for_read = Arc::clone(&clipboard);
            let clipboard_for_write = Arc::clone(&clipboard);
            let clipboard_for_paste = Arc::clone(&clipboard);
            let pasted = Arc::clone(&pasted);

            handles.push(tokio::spawn(async move {
                run_temporary_text_transaction(
                    &transaction,
                    text,
                    Duration::from_millis(1),
                    Duration::from_millis(1),
                    move || Ok(clipboard_for_read.lock().unwrap().clone()),
                    move |value| {
                        *clipboard_for_write.lock().unwrap() = value.to_string();
                        Ok(())
                    },
                    move || async move {
                        pasted
                            .lock()
                            .unwrap()
                            .push(clipboard_for_paste.lock().unwrap().clone());
                        tokio::time::sleep(Duration::from_millis(2)).await;
                        Ok(())
                    },
                )
                .await
            }));
        }

        for handle in handles {
            handle.await.unwrap().unwrap();
        }

        let pasted = pasted.lock().unwrap().clone();
        assert_eq!(pasted.len(), texts.len());
        for text in texts {
            assert!(pasted.contains(&text.to_string()));
        }
        assert_eq!(*clipboard.lock().unwrap(), "original Ω\nline two");
    }

    #[tokio::test]
    async fn two_overlapping_transactions_preserve_unicode_and_multiline_snapshot() {
        assert_overlapping_transactions_preserve_snapshot(&["first 🌍\nline", "second 世界\nline"])
            .await;
    }

    #[tokio::test]
    async fn three_overlapping_transactions_preserve_unicode_and_multiline_snapshot() {
        assert_overlapping_transactions_preserve_snapshot(&[
            "first 🌍\nline",
            "second 世界\nline",
            "third á\nline",
        ])
        .await;
    }

    #[tokio::test]
    async fn read_failure_aborts_before_clipboard_write_or_paste() {
        let transaction = AsyncMutex::new(());
        let writes = Arc::new(Mutex::new(Vec::new()));
        let paste_calls = Arc::new(Mutex::new(0usize));
        let writes_for_action = Arc::clone(&writes);
        let calls_for_action = Arc::clone(&paste_calls);

        let error = run_temporary_text_transaction(
            &transaction,
            "replacement",
            Duration::ZERO,
            Duration::ZERO,
            || Err("clipboard unavailable".to_string()),
            move |value| {
                writes_for_action.lock().unwrap().push(value.to_string());
                Ok(())
            },
            move || async move {
                *calls_for_action.lock().unwrap() += 1;
                Ok(())
            },
        )
        .await
        .unwrap_err();

        assert!(error.contains("Failed to read clipboard"));
        assert!(writes.lock().unwrap().is_empty());
        assert_eq!(*paste_calls.lock().unwrap(), 0);
    }

    #[tokio::test]
    async fn write_failure_aborts_paste_and_restore_failure_is_reported() {
        let transaction = AsyncMutex::new(());
        let paste_calls = Arc::new(Mutex::new(0usize));
        let calls_for_action = Arc::clone(&paste_calls);

        let write_error = run_temporary_text_transaction(
            &transaction,
            "replacement",
            Duration::ZERO,
            Duration::ZERO,
            || Ok("original".to_string()),
            |_value| Err("clipboard locked".to_string()),
            move || async move {
                *calls_for_action.lock().unwrap() += 1;
                Ok(())
            },
        )
        .await
        .unwrap_err();

        assert!(write_error.contains("Failed to set clipboard"));
        assert_eq!(*paste_calls.lock().unwrap(), 0);

        let writes = Arc::new(Mutex::new(0usize));
        let writes_for_action = Arc::clone(&writes);
        let restore_error = run_temporary_text_transaction(
            &transaction,
            "replacement",
            Duration::ZERO,
            Duration::ZERO,
            || Ok("original".to_string()),
            move |_value| {
                let mut writes = writes_for_action.lock().unwrap();
                *writes += 1;
                if *writes == 2 {
                    Err("clipboard locked".to_string())
                } else {
                    Ok(())
                }
            },
            || async { Ok(()) },
        )
        .await
        .unwrap_err();

        assert!(restore_error.contains("Failed to restore clipboard"));
    }
}
