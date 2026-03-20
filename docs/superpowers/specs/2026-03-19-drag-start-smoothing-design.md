# Drag Start Smoothing Design

**Problem:** Dragging the widget window and the settings window feels unreliable at the start, especially when beginning a drag gesture.

**Scope:** Keep the current drag behavior and interaction model, but make drag initiation more reliable and smoother.

## Decisions

1. Keep widget interactions unchanged:
   - Left mouse button drags the widget
   - Right mouse button opens settings
2. Add a short widget drag grace window so cursor ignore polling cannot immediately disable interaction right as dragging starts.
3. Replace the settings header's passive drag-region markup with explicit manual drag start on mouse down.

## Implementation Shape

- Add a small widget drag-start grace timer in `src/App.tsx` and make the widget interactivity polling respect it.
- Start widget dragging in the capture phase so the drag request is sent before other bubbling work.
- Start settings window dragging manually from the header, while still excluding the close button.
- Extend `src/widgetBehavior.test.mjs` with coverage for the settings header drag path and the widget drag entry point.

## Verification

- Run the frontend node tests covering widget behavior.
- Run a production frontend build to catch TypeScript or JSX regressions.
