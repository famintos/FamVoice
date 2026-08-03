use serde::Serialize;
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use tokio::sync::{Mutex as AsyncMutex, MutexGuard as AsyncMutexGuard};

pub type SessionId = u64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DictationPhase {
    Idle,
    Recording(SessionId),
    Transcribing(SessionId),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct DictationActivity {
    pub active: bool,
    pub recording: bool,
    pub transcribing: bool,
}

struct CoordinatorInner {
    next_session_id: SessionId,
    latest_session_id: Option<SessionId>,
    phase: DictationPhase,
    transcriptions_in_flight: HashSet<SessionId>,
}

pub struct DictationCoordinatorState {
    inner: Mutex<CoordinatorInner>,
    hotkey_pressed: AtomicBool,
    operation_gate: AsyncMutex<()>,
}

impl Default for DictationCoordinatorState {
    fn default() -> Self {
        Self {
            inner: Mutex::new(CoordinatorInner {
                next_session_id: 1,
                latest_session_id: None,
                phase: DictationPhase::Idle,
                transcriptions_in_flight: HashSet::new(),
            }),
            hotkey_pressed: AtomicBool::new(false),
            operation_gate: AsyncMutex::new(()),
        }
    }
}

impl DictationCoordinatorState {
    pub async fn lock_operation(&self) -> AsyncMutexGuard<'_, ()> {
        self.operation_gate.lock().await
    }

    pub fn begin_recording(&self) -> Result<SessionId, String> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| "Dictation session lock poisoned".to_string())?;

        if matches!(inner.phase, DictationPhase::Recording(_)) {
            return Err("A recording is already active".to_string());
        }

        let session_id = inner.next_session_id;
        inner.next_session_id = inner
            .next_session_id
            .checked_add(1)
            .ok_or_else(|| "Dictation session id limit reached".to_string())?;
        inner.latest_session_id = Some(session_id);
        inner.phase = DictationPhase::Recording(session_id);
        Ok(session_id)
    }

    pub fn begin_transcription(&self, session_id: SessionId) -> Result<(), String> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| "Dictation session lock poisoned".to_string())?;

        if inner.phase != DictationPhase::Recording(session_id) {
            return Err("The recording session is no longer active".to_string());
        }

        inner.phase = DictationPhase::Transcribing(session_id);
        inner.transcriptions_in_flight.insert(session_id);
        Ok(())
    }

    pub fn begin_retry_transcription(&self) -> Result<SessionId, String> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| "Dictation session lock poisoned".to_string())?;

        let has_active_phase = !matches!(inner.phase, DictationPhase::Idle);
        if has_active_phase || !inner.transcriptions_in_flight.is_empty() {
            return Err("A dictation is already active".to_string());
        }

        let session_id = inner.next_session_id;
        inner.next_session_id = inner
            .next_session_id
            .checked_add(1)
            .ok_or_else(|| "Dictation session id limit reached".to_string())?;
        inner.latest_session_id = Some(session_id);
        inner.phase = DictationPhase::Transcribing(session_id);
        inner.transcriptions_in_flight.insert(session_id);
        Ok(session_id)
    }

    pub fn should_deliver(&self, session_id: SessionId) -> bool {
        self.inner.lock().is_ok_and(|inner| {
            inner.latest_session_id == Some(session_id)
                && inner.phase == DictationPhase::Transcribing(session_id)
        })
    }

    pub fn finish_session(&self, session_id: SessionId) -> bool {
        let Ok(mut inner) = self.inner.lock() else {
            return false;
        };
        let was_active = matches!(
            inner.phase,
            DictationPhase::Recording(id) | DictationPhase::Transcribing(id) if id == session_id
        ) || inner.transcriptions_in_flight.contains(&session_id);
        let was_latest = was_active && inner.latest_session_id == Some(session_id);
        inner.transcriptions_in_flight.remove(&session_id);
        if matches!(
            inner.phase,
            DictationPhase::Recording(id) | DictationPhase::Transcribing(id) if id == session_id
        ) {
            inner.phase = DictationPhase::Idle;
        }
        was_latest
    }

    pub fn fail_recording(&self, session_id: SessionId) -> bool {
        let Ok(mut inner) = self.inner.lock() else {
            return false;
        };
        if inner.latest_session_id != Some(session_id)
            || inner.phase != DictationPhase::Recording(session_id)
        {
            return false;
        }

        inner.phase = DictationPhase::Idle;
        true
    }

    pub fn current_recording_session(&self) -> Option<SessionId> {
        let inner = self.inner.lock().ok()?;
        match inner.phase {
            DictationPhase::Recording(session_id) => Some(session_id),
            DictationPhase::Idle | DictationPhase::Transcribing(_) => None,
        }
    }

    pub fn activity(&self) -> DictationActivity {
        let Ok(inner) = self.inner.lock() else {
            return DictationActivity {
                active: true,
                recording: false,
                transcribing: true,
            };
        };
        let recording = matches!(inner.phase, DictationPhase::Recording(_));
        let transcribing = !inner.transcriptions_in_flight.is_empty();
        DictationActivity {
            active: recording || transcribing,
            recording,
            transcribing,
        }
    }

    pub fn mark_hotkey_pressed(&self) -> bool {
        self.hotkey_pressed
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    }

    pub fn mark_hotkey_released(&self) -> bool {
        self.hotkey_pressed
            .compare_exchange(true, false, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn session_ids_are_monotonic_and_a_new_recording_supersedes_transcription() {
        let coordinator = DictationCoordinatorState::default();
        let first = coordinator.begin_recording().unwrap();
        coordinator.begin_transcription(first).unwrap();
        let second = coordinator.begin_recording().unwrap();

        assert!(second > first);
        assert!(!coordinator.should_deliver(first));
        assert_eq!(coordinator.current_recording_session(), Some(second));
    }

    #[tokio::test]
    async fn blocked_first_api_and_out_of_order_responses_only_deliver_latest_session() {
        let coordinator = Arc::new(DictationCoordinatorState::default());
        let first = coordinator.begin_recording().unwrap();
        coordinator.begin_transcription(first).unwrap();
        let (release_first, wait_first) = tokio::sync::oneshot::channel::<()>();

        let first_coordinator = Arc::clone(&coordinator);
        let first_task = tokio::spawn(async move {
            wait_first.await.unwrap();
            let _gate = first_coordinator.lock_operation().await;
            let should_deliver = first_coordinator.should_deliver(first);
            first_coordinator.finish_session(first);
            should_deliver
        });

        let second = {
            let _gate = coordinator.lock_operation().await;
            coordinator.begin_recording().unwrap()
        };
        coordinator.begin_transcription(second).unwrap();
        let second_should_deliver = {
            let _gate = coordinator.lock_operation().await;
            let should_deliver = coordinator.should_deliver(second);
            coordinator.finish_session(second);
            should_deliver
        };

        release_first.send(()).unwrap();

        assert!(second_should_deliver);
        assert!(!first_task.await.unwrap());
    }

    #[test]
    fn microphone_change_stays_blocked_until_all_transcriptions_finish() {
        let coordinator = DictationCoordinatorState::default();
        let first = coordinator.begin_recording().unwrap();
        coordinator.begin_transcription(first).unwrap();
        let second = coordinator.begin_recording().unwrap();
        assert!(coordinator.activity().active);

        coordinator.fail_recording(second);
        assert!(coordinator.activity().active);
        assert!(coordinator.activity().transcribing);

        coordinator.finish_session(first);
        assert!(!coordinator.activity().active);
    }

    #[test]
    fn hotkey_press_state_is_independent_from_stream_health() {
        let coordinator = DictationCoordinatorState::default();

        assert!(coordinator.mark_hotkey_pressed());
        assert!(!coordinator.mark_hotkey_pressed());
        assert!(coordinator.mark_hotkey_released());
        assert!(!coordinator.mark_hotkey_released());
    }

    #[test]
    fn retry_transcription_is_a_first_class_session() {
        let coordinator = DictationCoordinatorState::default();

        let retry = coordinator.begin_retry_transcription().unwrap();

        assert!(coordinator.should_deliver(retry));
        assert_eq!(
            coordinator.activity(),
            DictationActivity {
                active: true,
                recording: false,
                transcribing: true,
            }
        );
        assert!(coordinator.finish_session(retry));
        assert!(!coordinator.activity().active);
    }

    #[test]
    fn retry_is_rejected_while_recording_is_active() {
        let coordinator = DictationCoordinatorState::default();
        let recording = coordinator.begin_recording().unwrap();

        assert_eq!(
            coordinator.begin_retry_transcription().unwrap_err(),
            "A dictation is already active"
        );
        assert_eq!(coordinator.current_recording_session(), Some(recording));
    }

    #[test]
    fn retry_is_rejected_while_any_transcription_is_in_flight() {
        let coordinator = DictationCoordinatorState::default();
        let first = coordinator.begin_recording().unwrap();
        coordinator.begin_transcription(first).unwrap();
        let second = coordinator.begin_recording().unwrap();
        coordinator.fail_recording(second);

        assert!(coordinator.activity().transcribing);
        assert_eq!(
            coordinator.begin_retry_transcription().unwrap_err(),
            "A dictation is already active"
        );
        assert!(!coordinator.should_deliver(first));
        coordinator.finish_session(first);
        assert!(coordinator.begin_retry_transcription().is_ok());
    }
}
