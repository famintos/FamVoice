# Signal Console UI Refresh Design

## Goal

Refresh FamVoice's frontend so it feels like a compact desktop recording instrument instead of a generic dark utility panel.

The redesign should:

- make the live recording state the dominant surface in the main window
- establish a consistent `developer instrument` visual language across the main window, settings window, and widget
- keep the app calm while idle and more vivid only when recording or transcribing
- preserve the existing behavior, IPC flow, and lightweight desktop footprint

## Product Context

FamVoice is not a dashboard, editor, or chat app. It is a focused desktop dictation tool that the user opens briefly, triggers via hotkey, and expects to understand at a glance.

The current UI works, but it relies on generic dark-glass patterns:

- blue accents on dark backgrounds
- repeated rounded-card treatment
- weak hierarchy between live state, transcript feedback, history, and controls
- settings that feel visually detached from the product identity

That makes the app functional but forgettable. The redesign should make the product feel authored and specific without adding visual noise or slowing the interface down.

## Approved Direction

The user-approved direction is:

- visual tone: `developer instrument`
- dominant use case: `live recording state`
- motion and energy: `balanced signal`
- settings treatment: same family as the main UI, but quieter and denser
- design concept: `Signal Console`

`Signal Console` means the UI should resemble a compact recording/control surface:

- restrained when inactive
- sharply legible under load
- purposeful with warm signal accents
- denser and more operational than a consumer-style glass panel

## Scope

- Refresh the visual system used by `MainView`, `SettingsView`, `WidgetView`, `VoiceWave`, and shared app styles.
- Replace the current generic glass-heavy look with a more solid, instrument-like surface treatment.
- Rework hierarchy in the main window so recording/transcribing state is the visual focus.
- Restyle history into a denser utility log treatment.
- Restyle settings into a compact control-rack layout using the same visual family.
- Refine widget styling so it becomes the purest expression of the new system.
- Preserve all existing interaction flows, commands, events, and window behaviors.

## Non-goals

- Changing backend behavior, IPC commands, or persistence formats.
- Adding new product features or new settings sections.
- Rewriting the information architecture of settings beyond visual grouping and density improvements.
- Introducing complex animation systems, custom canvas rendering, or heavy visual effects.
- Making the UI feel like an audio-production suite or a neon futuristic dashboard.
- Turning the app into a bright/light-theme interface.

## Why This Path

FamVoice already has a strong product metaphor available: press, speak, release, done. The UI should reinforce that simple flow instead of diluting it with equal-weight surfaces and decorative chrome.

Using a warm signal palette tied to the existing logo and a more operational layout solves the largest problems with the current interface:

- stronger identity without a full rebrand
- clearer hierarchy in the main window
- better continuity between windows
- a more memorable design that avoids common AI-generated desktop UI defaults

## Design

### 1. Shared Visual System

The redesign should move from translucent glass cards to layered, mostly solid instrument surfaces.

Shared rules:

- Use deep graphite and ink-blue neutrals rather than pure black.
- Use subtle layer separation through temperature and value shifts instead of heavy blur.
- Keep corner radii controlled and purposeful; avoid oversized rounded panels.
- Use borders and inset highlights sparingly to define surfaces.
- Keep shadows soft and tinted, not muddy black.

The interface should feel engineered, compact, and stable rather than glossy.

### 2. Typography

Use `IBM Plex Sans` as the primary interface typeface and `IBM Plex Mono` for metadata and machine-like labels.

Type roles:

- app title, live status headings, and key action labels use `IBM Plex Sans`
- hotkeys, timestamps, version text, model names, and small status tokens use `IBM Plex Mono`
- body copy remains compact, screen-readable, and left-aligned where reading matters

Hierarchy rules:

- top-level active state copy must be meaningfully larger than tertiary labels
- small labels should rely on spacing and case, not just low contrast
- transcript and error readouts should look like operational output, not chat bubbles
- explanatory settings copy must visibly recede behind labels and values

Implementation constraint:

- bundle or locally ship the IBM Plex fonts with the app; do not rely on remote font fetching

### 3. Color And State Tokens

The palette should be a restrained dark scheme with warm signal accents.

Recommended token roles:

- base background: deep graphite / ink-blue
- raised surface: slightly lighter cool neutral
- border and dividers: slate-toned linework
- primary text: near-white with a slight cool bias
- secondary text: muted cool slate
- identity accent: burnt amber / signal orange
- recording state: brighter amber derived from the identity accent
- transcribing state: straw-gold signal distinct from recording
- success: restrained green
- error: rusted red

Behavior rules:

- idle state stays low-intensity and calm
- recording visibly intensifies the waveform and central stage
- transcribing shifts from capture energy to processing clarity
- success and error appear as deliberate state panels, not tiny inline dots
- accent color should be concentrated in meaningful UI points, not spread across every control

### 4. Main Window Layout

The main window should use three clear bands:

1. a thin command bar
2. a dominant center stage for live state
3. a lower utility rail for mode switching and contextual output

#### Command Bar

The top bar should remain compact and minimal:

- app identity on the left
- settings and window controls on the right
- lower visual weight than the center stage

It should feel like frame chrome, not a navigation header.

#### Center Stage

The center stage is the primary redesign target.

Expected behavior by state:

- idle: subdued waveform, low-contrast readiness signal, generous breathing room
- recording: larger and brighter waveform, stronger active field behind the waveform, clearer live-state copy
- transcribing: visible shift away from waveform-led capture into processing feedback
- success: concise success state with strong readability and controlled color use
- error: a deliberate error readout panel with icon plus message, not a small inline treatment

Transcript feedback should appear as an operational readout:

- left-aligned
- compact but legible
- visually connected to the current state
- styled as system output rather than a floating toast

### 5. Tabs And History

The record surface remains primary. History should feel like a secondary utility log.

Tab behavior and treatment:

- keep the record/history toggle compact and visually subordinate to the center stage
- preserve clear active-state indication without making tabs overly decorative

History styling:

- replace repeated rounded-card entries with denser log-like rows or restrained list items
- keep transcript text left-aligned and scannable
- show timestamps in `IBM Plex Mono`
- keep copy, re-paste, and delete actions visible enough to be discoverable without requiring a pure hover-only reveal
- use spacing and linework to separate rows instead of card stacks

Empty-state behavior:

- keep the empty state minimal and secondary
- avoid large decorative iconography competing with the main app purpose

### 6. Settings Window

Settings should feel like the same product family in a quieter, denser form.

Layout rules:

- organize sections as compact control bands with clearer rhythm between groups
- reduce decorative fills and visual noise
- rely more on alignment, spacing, and type hierarchy than on colored blocks
- preserve strong legibility for dense controls in a relatively small window

Control treatment:

- routine inputs stay mostly neutral
- focus, hover, and selected states use restrained amber accents
- explanatory copy is present but visually recessed
- destructive or warning states should be clear without dominating the entire window

Action treatment:

- primary actions such as `Save Changes` and `Update` can carry the warm identity accent
- secondary actions should remain visually quieter

### 7. Widget

The widget should be the most distilled version of the design system.

Widget rules:

- compact, dense, and hardware-like
- stronger silhouette than the current generic pill treatment
- waveform remains the main element
- text appears only when it communicates something necessary, such as an error or missing key state
- highlight behavior should read as a signal pulse rather than a decorative glow burst

The widget must still feel lightweight and easy to ignore when idle.

### 8. Motion And Feedback

Motion should clarify state transitions, not decorate them.

Rules:

- idle motion remains subtle
- recording animation can become more energetic, but should stay smooth and controlled
- transcribing should feel distinct from recording rather than sharing the same exact visual pulse
- repeated entrance animations on routine content should be reduced
- hover and focus effects should reflect UI importance; not every element needs the same animated treatment

Accessibility constraint:

- respect reduced-motion preferences for nonessential animation

### 9. Small-Footprint And Accessibility Constraints

The redesign must remain readable and usable in a compact desktop window.

Constraints:

- preserve the current small-footprint ergonomics of the main window and widget
- avoid tiny low-contrast text in operational areas
- keep keyboard focus states visible
- maintain readable contrast between text and surfaces
- ensure history actions and settings controls remain usable without relying solely on hover

## Implementation Notes

- The redesign is frontend-only.
- Keep existing component boundaries unless a small refactor materially improves styling clarity.
- Shared tokens should move into the app-wide stylesheet/theme layer instead of being repeated ad hoc in component class strings.
- The design should reduce reliance on `backdrop-blur` and translucent black overlays.
- If a new font-loading dependency is introduced, it must be local to the app bundle.

## Verification

Implementation verification should cover both behavior preservation and visual intent.

Minimum checks:

- existing behavior tests remain green unless selectors or assumptions need intentional updates
- main window states are manually verified in idle, recording, transcribing, success, and error modes
- history remains usable with mouse and keyboard in the refreshed layout
- settings remain readable and navigable in the smaller window footprint
- widget idle, recording, missing-key, and error states are manually verified
- reduced-motion behavior is checked for nonessential effects

## Acceptance Criteria

- The main window reads as a recording-focused instrument surface within one glance.
- The live state is visually dominant over history and utility controls.
- The app no longer relies on generic blue-on-dark glass styling.
- Typography uses `IBM Plex Sans` and `IBM Plex Mono` in purposeful roles.
- Warm signal accents are used deliberately and intensify only during active states.
- History is presented as a denser utility log rather than a stack of generic cards.
- Settings feels consistent with the main app while remaining quieter and denser.
- The widget reflects the same system in a more distilled form.
- No backend behavior or IPC surface changes are required for the redesign.
