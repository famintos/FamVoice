use serde::Serialize;
use std::sync::{Arc, Mutex, Weak};
use std::time::{Duration, Instant};

pub const MAX_RETRY_AUDIO_BYTES: usize = 10 * 1024 * 1024;
pub const RETRY_AUDIO_TTL: Duration = Duration::from_secs(2 * 60);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RetryAudioFormat {
    Flac,
    Wav,
}

impl RetryAudioFormat {
    pub fn mime_type(self) -> &'static str {
        match self {
            Self::Flac => "audio/flac",
            Self::Wav => "audio/wav",
        }
    }

    pub fn file_name(self) -> &'static str {
        match self {
            Self::Flac => "audio.flac",
            Self::Wav => "audio.wav",
        }
    }
}

/// The public retry state deliberately exposes no audio bytes, size, format, or
/// capture timestamp. Those details remain confined to process memory.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RetryAudioStatus {
    pub available: bool,
}

pub struct RetryAudio {
    bytes: Vec<u8>,
    format: RetryAudioFormat,
    audio_duration: Duration,
}

impl RetryAudio {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn format(&self) -> RetryAudioFormat {
        self.format
    }

    pub fn audio_duration(&self) -> Duration {
        self.audio_duration
    }

    #[cfg(test)]
    pub fn into_parts(mut self) -> (Vec<u8>, &'static str, &'static str, Duration) {
        let bytes = std::mem::take(&mut self.bytes);
        (
            bytes,
            self.format.mime_type(),
            self.format.file_name(),
            self.audio_duration,
        )
    }
}

impl Drop for RetryAudio {
    fn drop(&mut self) {
        scrub_bytes(&mut self.bytes);
    }
}

struct StoredRetryAudio {
    generation: u64,
    expires_at: Instant,
    audio: RetryAudio,
}

struct RetryAudioInner {
    generation: u64,
    audio: Option<StoredRetryAudio>,
}

/// A process-local, single-item cache for the most recent failed dictation.
///
/// Expiration timers hold only a weak reference. Dropping the state therefore
/// clears the retained audio immediately instead of extending its lifetime to
/// the timer deadline.
pub struct RetryAudioState {
    inner: Arc<Mutex<RetryAudioInner>>,
    max_bytes: usize,
    ttl: Duration,
}

impl Default for RetryAudioState {
    fn default() -> Self {
        Self::new()
    }
}

impl RetryAudioState {
    pub fn new() -> Self {
        Self::with_limits(MAX_RETRY_AUDIO_BYTES, RETRY_AUDIO_TTL)
    }

    fn with_limits(max_bytes: usize, ttl: Duration) -> Self {
        Self {
            inner: Arc::new(Mutex::new(RetryAudioInner {
                generation: 0,
                audio: None,
            })),
            max_bytes,
            ttl,
        }
    }

    pub fn store(
        &self,
        bytes: Vec<u8>,
        format: RetryAudioFormat,
        audio_duration: Duration,
    ) -> Result<(), String> {
        let audio = RetryAudio {
            bytes,
            format,
            audio_duration,
        };
        if audio.bytes.is_empty() {
            self.discard();
            return Err("Failed dictation audio is empty".to_string());
        }
        if audio.bytes.len() > self.max_bytes {
            self.discard();
            return Err(format!(
                "Failed dictation audio exceeds the {} MiB retry limit",
                self.max_bytes / (1024 * 1024)
            ));
        }

        let generation = {
            let mut inner = self
                .inner
                .lock()
                .map_err(|_| "Retry audio lock poisoned".to_string())?;
            inner.generation = inner
                .generation
                .checked_add(1)
                .ok_or_else(|| "Retry audio generation limit reached".to_string())?;
            let generation = inner.generation;
            inner.audio = Some(StoredRetryAudio {
                generation,
                expires_at: Instant::now() + self.ttl,
                audio,
            });
            generation
        };

        schedule_expiration(&self.inner, generation, self.ttl);
        Ok(())
    }

    pub fn status(&self) -> RetryAudioStatus {
        let Ok(mut inner) = self.inner.lock() else {
            return RetryAudioStatus::default();
        };
        remove_if_expired(&mut inner, Instant::now());
        RetryAudioStatus {
            available: inner.audio.is_some(),
        }
    }

    /// Removes and returns the cached audio exactly once. A stale item is
    /// cleared rather than handed to the caller.
    pub fn take(&self) -> Option<RetryAudio> {
        let mut inner = self.inner.lock().ok()?;
        remove_if_expired(&mut inner, Instant::now());
        let audio = inner.audio.take()?.audio;
        advance_generation(&mut inner);
        Some(audio)
    }

    pub fn discard(&self) -> bool {
        self.clear_current()
    }

    #[cfg(test)]
    pub fn expire(&self) -> bool {
        self.clear_current()
    }

    fn clear_current(&self) -> bool {
        let Ok(mut inner) = self.inner.lock() else {
            return false;
        };
        let removed = inner.audio.take().is_some();
        if removed {
            advance_generation(&mut inner);
        }
        removed
    }
}

impl Drop for RetryAudioState {
    fn drop(&mut self) {
        let Ok(mut inner) = self.inner.lock() else {
            return;
        };
        inner.audio.take();
        advance_generation(&mut inner);
    }
}

fn advance_generation(inner: &mut RetryAudioInner) {
    inner.generation = inner.generation.saturating_add(1);
}

fn remove_if_expired(inner: &mut RetryAudioInner, now: Instant) -> bool {
    if inner
        .audio
        .as_ref()
        .is_some_and(|stored| now >= stored.expires_at)
    {
        inner.audio.take();
        advance_generation(inner);
        return true;
    }
    false
}

fn schedule_expiration(inner: &Arc<Mutex<RetryAudioInner>>, generation: u64, ttl: Duration) {
    let Ok(runtime) = tokio::runtime::Handle::try_current() else {
        // `status` and `take` still enforce the deadline when no async runtime
        // is active (for example, during a synchronous construction test).
        return;
    };
    let weak_inner = Arc::downgrade(inner);
    runtime.spawn(async move {
        tokio::time::sleep(ttl).await;
        expire_generation(&weak_inner, generation);
    });
}

fn expire_generation(inner: &Weak<Mutex<RetryAudioInner>>, generation: u64) -> bool {
    let Some(inner) = inner.upgrade() else {
        return false;
    };
    let Ok(mut inner) = inner.lock() else {
        return false;
    };
    if inner
        .audio
        .as_ref()
        .is_none_or(|stored| stored.generation != generation)
    {
        return false;
    }
    inner.audio.take();
    true
}

fn scrub_bytes(bytes: &mut Vec<u8>) {
    bytes.fill(0);
    // Make the clearing operation observable so the compiler cannot discard it
    // merely because the allocation is about to be released.
    std::hint::black_box(bytes.as_mut_slice());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state_with_short_ttl() -> RetryAudioState {
        RetryAudioState::with_limits(8, Duration::from_millis(20))
    }

    fn current_generation(state: &RetryAudioState) -> u64 {
        state.inner.lock().unwrap().generation
    }

    #[tokio::test]
    async fn stores_only_the_latest_audio_and_exposes_no_sensitive_status_metadata() {
        let state = state_with_short_ttl();
        state
            .store(
                vec![1, 2, 3],
                RetryAudioFormat::Flac,
                Duration::from_secs(1),
            )
            .unwrap();
        state
            .store(vec![4, 5], RetryAudioFormat::Wav, Duration::from_secs(2))
            .unwrap();

        assert_eq!(state.status(), RetryAudioStatus { available: true });
        assert_eq!(
            serde_json::to_value(state.status()).unwrap(),
            serde_json::json!({ "available": true })
        );

        let audio = state.take().unwrap();
        assert_eq!(audio.bytes(), &[4, 5]);
        assert_eq!(audio.format(), RetryAudioFormat::Wav);
        assert_eq!(audio.audio_duration(), Duration::from_secs(2));
    }

    #[tokio::test]
    async fn size_limit_is_inclusive_and_invalid_audio_invalidates_the_previous_item() {
        let state = RetryAudioState::with_limits(4, RETRY_AUDIO_TTL);
        state
            .store(vec![1; 4], RetryAudioFormat::Flac, Duration::from_secs(1))
            .unwrap();
        assert!(state.status().available);

        let error = state
            .store(vec![2; 5], RetryAudioFormat::Wav, Duration::from_secs(2))
            .unwrap_err();
        assert!(error.contains("exceeds"));
        assert!(!state.status().available);

        state
            .store(vec![3], RetryAudioFormat::Wav, Duration::from_secs(3))
            .unwrap();
        assert!(state
            .store(Vec::new(), RetryAudioFormat::Wav, Duration::from_secs(4))
            .unwrap_err()
            .contains("empty"));
        assert!(!state.status().available);
    }

    #[tokio::test]
    async fn take_is_single_use_and_returns_provider_upload_metadata() {
        let state = state_with_short_ttl();
        state
            .store(
                vec![9, 8, 7],
                RetryAudioFormat::Flac,
                Duration::from_millis(375),
            )
            .unwrap();

        let (bytes, mime_type, file_name, audio_duration) = state.take().unwrap().into_parts();
        assert_eq!(bytes, vec![9, 8, 7]);
        assert_eq!(mime_type, "audio/flac");
        assert_eq!(file_name, "audio.flac");
        assert_eq!(audio_duration, Duration::from_millis(375));
        assert!(state.take().is_none());
        assert!(!state.status().available);
    }

    #[tokio::test]
    async fn discard_and_expire_are_idempotent() {
        let state = state_with_short_ttl();
        assert!(!state.discard());
        assert!(!state.expire());

        state
            .store(vec![1], RetryAudioFormat::Flac, Duration::from_secs(1))
            .unwrap();
        assert!(state.discard());
        assert!(!state.discard());

        state
            .store(vec![2], RetryAudioFormat::Wav, Duration::from_secs(2))
            .unwrap();
        assert!(state.expire());
        assert!(!state.expire());
    }

    #[tokio::test]
    async fn status_and_take_lazily_reject_expired_audio() {
        let state = RetryAudioState::with_limits(8, Duration::ZERO);
        state
            .store(vec![1], RetryAudioFormat::Flac, Duration::from_secs(1))
            .unwrap();

        assert!(!state.status().available);
        assert!(state.take().is_none());
        assert!(!state.expire());
    }

    #[tokio::test]
    async fn scheduled_expiration_removes_the_current_audio() {
        let state = state_with_short_ttl();
        state
            .store(vec![1], RetryAudioFormat::Flac, Duration::from_secs(1))
            .unwrap();

        tokio::time::sleep(Duration::from_millis(50)).await;

        assert!(!state.status().available);
        assert!(state.take().is_none());
    }

    #[tokio::test]
    async fn an_old_timer_generation_cannot_remove_a_newer_item() {
        let state = state_with_short_ttl();
        state
            .store(vec![1], RetryAudioFormat::Flac, Duration::from_secs(1))
            .unwrap();
        let old_generation = current_generation(&state);
        state
            .store(vec![2], RetryAudioFormat::Wav, Duration::from_secs(2))
            .unwrap();

        assert!(!expire_generation(
            &Arc::downgrade(&state.inner),
            old_generation
        ));
        assert!(state.status().available);
        assert_eq!(state.take().unwrap().bytes(), &[2]);
    }

    #[test]
    fn synchronous_callers_still_enforce_expiration_without_a_runtime() {
        let state = RetryAudioState::with_limits(8, Duration::ZERO);
        state
            .store(vec![1], RetryAudioFormat::Flac, Duration::from_secs(1))
            .unwrap();

        assert!(!state.status().available);
    }

    #[test]
    fn default_policy_uses_the_phase_six_privacy_limits() {
        let state = RetryAudioState::new();
        assert_eq!(state.max_bytes, 10 * 1024 * 1024);
        assert_eq!(state.ttl, Duration::from_secs(120));
        assert!(!state.status().available);
    }

    #[test]
    fn sensitive_byte_cleanup_overwrites_the_full_buffer() {
        let mut bytes = vec![0xA5; 32];

        scrub_bytes(&mut bytes);

        assert_eq!(bytes, vec![0; 32]);
    }
}
