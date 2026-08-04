//! Tray icon state machine.
//!
//! The FamVoice mark is two amplitude bars followed by two transcript lines. The
//! tray drives each half from the pipeline it stands for: the bars follow live
//! microphone level while recording, the lines fill while the transcript is being
//! produced. Nothing here animates unless work is actually happening.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::image::Image;
use tauri::{include_image, AppHandle, Listener, Manager, State};

pub const TRAY_ID: &str = "famvoice";

/// Microphone level arrives at roughly 62 Hz. The tray only needs to move when the
/// quantised level changes, and never faster than this.
const LEVEL_THROTTLE: Duration = Duration::from_millis(80);
/// Transcription exposes no progress, so the lines cycle at a steady rate to report
/// "working" without implying a percentage.
const TRANSCRIBING_FRAME_INTERVAL: Duration = Duration::from_millis(280);
const RECORDING_LEVELS: u8 = 4;
const TRANSCRIBING_FRAMES: u8 = 3;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Frame {
    Idle,
    Recording(u8),
    Transcribing(u8),
    Success,
}

/// The frame the tray starts on, before any pipeline activity.
pub fn idle_image() -> Image<'static> {
    include_image!("./icons/tray-idle.png")
}

fn image_for(frame: Frame) -> Image<'static> {
    match frame {
        Frame::Idle => include_image!("./icons/tray-idle.png"),
        Frame::Recording(level) => match level {
            0 => include_image!("./icons/tray-rec-0.png"),
            1 => include_image!("./icons/tray-rec-1.png"),
            2 => include_image!("./icons/tray-rec-2.png"),
            _ => include_image!("./icons/tray-rec-3.png"),
        },
        Frame::Transcribing(step) => match step {
            0 => include_image!("./icons/tray-tr-0.png"),
            1 => include_image!("./icons/tray-tr-1.png"),
            _ => include_image!("./icons/tray-tr-2.png"),
        },
        Frame::Success => include_image!("./icons/tray-success.png"),
    }
}

pub struct TrayState {
    current: Mutex<Frame>,
    /// Bumped on every pipeline stage change so stale animation loops retire.
    generation: AtomicU64,
    last_level_paint: Mutex<Instant>,
}

impl Default for TrayState {
    fn default() -> Self {
        Self {
            current: Mutex::new(Frame::Idle),
            generation: AtomicU64::new(0),
            last_level_paint: Mutex::new(Instant::now()),
        }
    }
}

impl TrayState {
    fn begin_stage(&self) -> u64 {
        self.generation.fetch_add(1, Ordering::SeqCst) + 1
    }

    fn is_current_stage(&self, generation: u64) -> bool {
        self.generation.load(Ordering::SeqCst) == generation
    }
}

/// Repaints under a single lock, so a late microphone sample can never overwrite a
/// newer pipeline stage. `next` returns `None` to leave the mark alone.
fn paint_with<F>(app: &AppHandle, next: F)
where
    F: FnOnce(Frame) -> Option<Frame>,
{
    let state: State<TrayState> = app.state();
    let frame = {
        let Ok(mut current) = state.current.lock() else {
            return;
        };
        let Some(frame) = next(*current) else {
            return;
        };
        if frame == *current {
            return;
        }
        *current = frame;
        frame
    };

    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let _ = tray.set_icon(Some(image_for(frame)));
    }
}

fn paint(app: &AppHandle, frame: Frame) {
    paint_with(app, |_| Some(frame));
}

fn recording_frame(level: f64) -> Frame {
    let step = (level.clamp(0.0, 1.0) * f64::from(RECORDING_LEVELS)) as u8;
    Frame::Recording(step.min(RECORDING_LEVELS - 1))
}

fn on_status(app: &AppHandle, status: &str) {
    let generation = {
        let state: State<TrayState> = app.state();
        state.begin_stage()
    };

    match status {
        "recording" => paint(app, Frame::Recording(0)),
        "transcribing" => {
            paint(app, Frame::Transcribing(0));
            animate_transcribing(app.clone(), generation);
        }
        "success" => paint(app, Frame::Success),
        // "error" and "idle" both land the mark back at rest. An error is reported in
        // the window with a recovery path; a 32px glyph cannot do that job.
        _ => paint(app, Frame::Idle),
    }
}

fn animate_transcribing(app: AppHandle, generation: u64) {
    tauri::async_runtime::spawn(async move {
        let mut step: u8 = 0;
        loop {
            tokio::time::sleep(TRANSCRIBING_FRAME_INTERVAL).await;

            {
                let state: State<TrayState> = app.state();
                if !state.is_current_stage(generation) {
                    return;
                }
            }

            step = (step + 1) % TRANSCRIBING_FRAMES;
            paint(&app, Frame::Transcribing(step));
        }
    });
}

fn on_level(app: &AppHandle, level: f64) {
    {
        let state: State<TrayState> = app.state();
        let Ok(mut last) = state.last_level_paint.lock() else {
            return;
        };
        if last.elapsed() < LEVEL_THROTTLE {
            return;
        }
        *last = Instant::now();
    }

    paint_with(app, |current| match current {
        Frame::Recording(_) => Some(recording_frame(level)),
        _ => None,
    });
}

/// Subscribes the tray to the pipeline. Call once, after the tray is built.
pub fn wire(app: &AppHandle) {
    app.manage(TrayState::default());

    let status_app = app.clone();
    app.listen("status", move |event| {
        if let Ok(status) = serde_json::from_str::<String>(event.payload()) {
            on_status(&status_app, &status);
        }
    });

    let level_app = app.clone();
    app.listen("mic-level", move |event| {
        if let Ok(level) = serde_json::from_str::<f64>(event.payload()) {
            on_level(&level_app, level);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quiet_input_uses_the_lowest_bar_frame() {
        assert_eq!(recording_frame(0.0), Frame::Recording(0));
        assert_eq!(recording_frame(0.24), Frame::Recording(0));
    }

    #[test]
    fn loud_input_saturates_at_the_full_mark_geometry() {
        assert_eq!(recording_frame(1.0), Frame::Recording(3));
        assert_eq!(recording_frame(5.0), Frame::Recording(3));
    }

    #[test]
    fn level_is_spread_across_every_available_frame() {
        let frames: Vec<Frame> = [0.1, 0.3, 0.6, 0.9]
            .iter()
            .map(|l| recording_frame(*l))
            .collect();

        assert_eq!(
            frames,
            vec![
                Frame::Recording(0),
                Frame::Recording(1),
                Frame::Recording(2),
                Frame::Recording(3),
            ]
        );
    }

    #[test]
    fn negative_level_cannot_underflow_the_frame_index() {
        assert_eq!(recording_frame(-1.0), Frame::Recording(0));
    }
}
