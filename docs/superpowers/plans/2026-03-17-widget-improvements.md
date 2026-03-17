# Widget and Audio Silence Improvements Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Improve widget mode by making its window size fit its content exactly (preventing transparent areas from blocking clicks) and add silence detection (RMS-based) to avoid sending empty audio to the transcription API.

**Architecture:** 
1. React (Frontend): Add a `ResizeObserver` to the main element in widget mode to measure its actual width/height and dynamically call `appWindow.setSize` via Tauri API to fit the exact content. Also adjust the `App.css` or classes to reduce/remove large shadow spread in widget mode so the bounds can be tighter.
2. Rust (Backend): In `lib.rs` `stop_recording_cmd`, before encoding to WAV and calling the API, calculate the RMS (Root Mean Square) volume of the captured `i16` audio samples. If the RMS is below a certain threshold (e.g., ~50.0), abort the API call, return an error or specific status to the frontend indicating "No voice detected", and emit a `status` update to reset the UI.

**Tech Stack:** React, Tauri API (Window Size), Rust (Audio processing)

---

### Task 1: Add Silence Detection (RMS) in Rust Backend

**Files:**
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Write the RMS check in `stop_recording_cmd`**
  In `src-tauri/src/lib.rs`, inside the `stop_recording_cmd` function, calculate the RMS of the samples directly after `audio::stop_recording`.

```rust
    let samples = match audio::stop_recording(&*audio_state).await {
        Some(s) => s,
        None => {
            eprintln!("[FamVoice] stop_recording returned None — was not recording");
            app.emit("status", "idle").unwrap();
            return Err("Not recording".into());
        }
    };

    // Calculate RMS volume to detect silence
    let mut sum_squares = 0.0;
    for &sample in &samples {
        let s = sample as f64;
        sum_squares += s * s;
    }
    let rms = (sum_squares / samples.len() as f64).sqrt();
    eprintln!("[FamVoice] Audio RMS volume: {:.2}", rms);

    if rms < 50.0 {
        eprintln!("[FamVoice] Silence detected, skipping transcription");
        app.emit("status", "error").unwrap();
        app.emit("transcript", "No voice detected").unwrap();
        
        let app_clone = app.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(2000)).await;
            let _ = app_clone.emit("status", "idle");
        });
        
        return Err("No voice detected".into());
    }

    // Encode to WAV in memory...
```

- [ ] **Step 2: Verify it compiles**
Run: `npm run tauri build -- -b none` (or `cargo check` in `src-tauri`)
Expected: Compiles successfully.

---

### Task 2: Dynamic Window Resizing in Widget Mode

**Files:**
- Modify: `src/App.tsx`
- Modify: `src/App.css` (if needed for shadow reduction)

- [ ] **Step 1: Reduce widget shadow**
In `src/App.tsx`, around line 489 in the widget mode `main` element, change `shadow-2xl` to `shadow-md` or remove it, to avoid having a huge transparent invisible box.
```tsx
        <main
          id="widget-container"
          data-tauri-drag-region
          className="relative flex items-center gap-3 px-4 py-2 bg-[#0f0f13] backdrop-blur-2xl rounded-full shadow-md border border-white/10 text-white"
          style={{ pointerEvents: "auto" }}
          // ...
```

- [ ] **Step 2: Add `ResizeObserver` to `App.tsx`**
Inside `MainView`, add a `useEffect` that attaches a `ResizeObserver` to the `#widget-container` when `settings?.widget_mode` is true.

```tsx
  useEffect(() => {
    if (!settings?.widget_mode) return;
    
    const container = document.getElementById("widget-container");
    if (!container) return;

    const observer = new ResizeObserver(async (entries) => {
      for (let entry of entries) {
        // Add a small margin (e.g., 2px) to prevent clipping if needed
        const width = Math.ceil(entry.contentRect.width) + 8;
        const height = Math.ceil(entry.contentRect.height) + 8;
        
        const size = new PhysicalSize(width, height);
        await appWindow.setMinSize(new PhysicalSize(1, 1));
        await appWindow.setMaxSize(new PhysicalSize(9999, 9999));
        await appWindow.setSize(size);
        await appWindow.setMinSize(size);
        await appWindow.setMaxSize(size);
      }
    });

    observer.observe(container);
    return () => observer.disconnect();
  }, [settings?.widget_mode, status, showWidgetMenu]); // Re-attach if mode or UI state that creates new elements changes
```

- [ ] **Step 3: Update existing size constraints**
Remove the fixed `160, 44` size from the existing `useEffect` in `MainView` for widget mode, so it doesn't fight with the `ResizeObserver`.

```tsx
        if (settings.widget_mode) {
          // Handled by ResizeObserver now, but set a base starting size
          const size = new PhysicalSize(160, 44);
          await appWindow.setSize(size);
        } else {
```

- [ ] **Step 4: Verify frontend build**
Run: `npm run build`
Expected: PASS

- [ ] **Step 5: Commit changes**
```bash
git add src-tauri/src/lib.rs src/App.tsx
git commit -m "feat: rms silence detection and dynamic widget resizing"
```