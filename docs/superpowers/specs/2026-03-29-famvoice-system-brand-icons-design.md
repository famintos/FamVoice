# FamVoice System Brand Icons Design

## Goal

Replace FamVoice's legacy system and bundle icons with Faminto brand-compliant assets so the executable, installer, taskbar, tray, and favicon all reflect the current Faminto identity.

The result should:

- use the Faminto solo mark where the brand guidelines require icon-only usage
- preserve `FamVoice` as the product name while adopting Faminto's shared symbol system
- align Windows-facing assets with the approved Faminto desktop and icon guidance
- remove remaining starter or legacy branding from bundle-level icon entry points

## Product Context

FamVoice is a product under the Faminto brand family. The Faminto guidelines define the symbol, color usage, background pairings, lockup rules, and desktop-specific tray behavior.

The current project is in an inconsistent state:

- in-app branding already uses the Faminto amber mark
- the bundle icon pack in `src-tauri/icons/` still contains older generated assets
- the web/dev favicon still points to `vite.svg`
- the tray currently reuses the default application icon instead of a tray-specific variant

That leaves the product visually split between the new Faminto language and older packaging defaults.

## Approved Direction

The user-approved direction is:

- keep `FamVoice` as the product identity
- use the Faminto brand guidelines as the source of truth
- replace only system-level and packaging icon assets
- do not replace functional UI icons such as settings, close, copy, or delete

## Scope

- Replace the source assets used for app icon generation with Faminto-compliant brand assets.
- Refresh the Tauri bundle icon pack in `src-tauri/icons/` for desktop and Windows packaging.
- Update the runtime tray icon behavior so it can use a tray-appropriate Faminto mark variant instead of blindly reusing the default app icon.
- Replace the current favicon entry in `index.html` with a Faminto-derived icon asset.
- Bring the necessary brand source assets into the FamVoice repo so the project no longer depends on an external brand folder at runtime or during future regeneration work.

## Non-goals

- Replacing Lucide-based functional UI icons in the React interface.
- Redesigning in-app headers, wordmarks, or widget composition.
- Renaming the app from `FamVoice` to `Faminto`.
- Changing updater, window, tray menu, or backend behavior beyond icon selection.
- Creating a new brand language outside the existing Faminto guidelines.

## Why This Path

The Faminto guidelines are explicit:

- icon-only contexts such as app icons and favicons should use the solo mark
- tray icons should use monochrome variants suited to OS chrome
- lockups are for branded name presentation, not for tiny system icons

Using the shared Faminto mark for system surfaces keeps the product consistent with the brand family while still allowing `FamVoice` to remain the visible product name in textual UI where appropriate.

## Design

### 1. Source Of Truth Assets

FamVoice should vendor the exact Faminto brand assets it needs into the repo.

Required source assets:

- the amber Faminto mark for primary app icon generation
- monochrome white and black Faminto mark variants for tray usage
- a checked-in icon source surface for the app icon plate, derived from the approved Faminto desktop examples

The app should not rely on `C:\Users\henri\Desktop\Faminto\FamBrand` after this work lands. Future icon refreshes should be possible from the repo alone.

### 2. Primary App Icon

The primary application icon should follow the approved `FamVoice` surface pairing from the guidelines:

- background plate: `#161B26` surface
- foreground mark: Faminto amber `#D17A28`
- shape: rounded square app plate suitable for Windows and desktop launcher contexts

This icon is the source for:

- `exe`
- installer
- taskbar / window icon
- Tauri bundle icon outputs
- Windows tile assets generated under `src-tauri/icons/`

The app icon should be optimized for small-size legibility and should not use the horizontal lockup.

### 3. Tray Icon

The tray icon should stop inheriting the default application icon.

Instead, tray rendering should use a brand-approved monochrome Faminto mark variant:

- white mark for dark tray contexts
- black mark for light tray contexts

Because tray icons have different legibility constraints than launcher icons, they should not reuse the colored rounded-square app icon. The implementation may choose the safer default variant per platform if dynamic theme switching is not practical in this pass, but it must still use a monochrome tray asset rather than the general app icon.

### 4. Favicon

The project favicon should stop pointing to Vite starter branding.

The replacement should be a Faminto solo mark asset appropriate for browser/tab display. This should be sourced from the same repo-vendored Faminto asset set used for the system icon refresh.

### 5. Generated Icon Pack

`src-tauri/icons/` should be treated as generated output, but the checked-in files must be refreshed so builds immediately use the new brand.

This includes:

- desktop PNG variants referenced by `tauri.conf.json`
- `icon.ico`
- `icon.icns`
- Windows square/store tile assets
- any generated Android and iOS launcher assets currently checked in under the same icon pack

Even if the user's immediate request is Windows-focused, the checked-in Tauri icon set should remain internally consistent after regeneration.

### 6. Runtime Integration

`tauri.conf.json` should continue to point at the standard generated bundle asset paths, but those files should now be Faminto-compliant.

The backend tray setup in `src-tauri/src/lib.rs` should be updated so tray icon creation uses an explicit tray asset instead of `app.default_window_icon()`.

Window-specific icon overrides are not required unless implementation reveals a platform-specific need. The default window icon path remains the correct source for main app windows.

## Error Handling And Constraints

- Do not introduce runtime failures if a tray-specific icon asset cannot be loaded; tray setup should fail gracefully or fall back to the default icon path in a controlled way.
- Keep icon generation and consumption deterministic; future builds should not depend on machine-specific paths outside the repo.
- Avoid manual one-off edits to only part of the icon set. The checked-in outputs should be regenerated coherently from the chosen source assets.

## Verification

Implementation verification should cover both asset integrity and runtime usage.

Minimum checks:

- confirm `index.html` no longer references `vite.svg`
- confirm `src-tauri/tauri.conf.json` still points at a valid regenerated icon set
- confirm the primary generated Windows assets in `src-tauri/icons/` changed to the new brand
- verify the tray path in `src-tauri/src/lib.rs` no longer depends solely on `default_window_icon()`
- run frontend build verification so the favicon and asset imports resolve
- run Tauri-side verification sufficient to confirm icon assets are accepted by the bundling pipeline

## Acceptance Criteria

- FamVoice uses a Faminto-compliant solo mark for system icon contexts.
- The `exe`, installer, taskbar/window icon, and generated Tauri bundle assets reflect the new Faminto icon treatment.
- The tray no longer reuses the default colored app icon and instead uses a monochrome Faminto mark variant.
- The favicon no longer uses Vite starter branding.
- The repo contains the required source assets for future icon regeneration without depending on the external FamBrand folder.
- No functional React UI icon set changes are included in this work.
