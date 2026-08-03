# Rust audit warning review

Reviewed on 2026-08-02 for the supported target `x86_64-pc-windows-msvc`.

## Result

`cargo audit --file src-tauri/Cargo.lock` reports zero vulnerabilities and 19 informational warnings: 17 unmaintained crates and two soundness advisories. The warnings are grouped below by reachability instead of being silently ignored.

The refreshed advisory database also exposed `RUSTSEC-2026-0186` in the target-conditional `enigo -> xkbcommon -> memmap2` lockfile chain. `cargo tree --target x86_64-pc-windows-msvc -i memmap2@0.9.11` confirms that chain is absent from the supported Windows target, but the warning was still removed because a compatible patch exists: the lockfile now resolves `memmap2 0.9.11`, the [first patched release](https://rustsec.org/advisories/RUSTSEC-2026-0186).

## Linux-only Tauri/GTK dependencies

The following 12 warnings are absent from `cargo tree --target x86_64-pc-windows-msvc` and enter only through Tauri's Linux GTK/WebKit backend:

- `RUSTSEC-2024-0411` — `gdkwayland-sys`
- `RUSTSEC-2024-0412` — `gdk`
- `RUSTSEC-2024-0413` — `atk`
- `RUSTSEC-2024-0414` — `gdkx11-sys`
- `RUSTSEC-2024-0415` — `gtk`
- `RUSTSEC-2024-0416` — `atk-sys`
- `RUSTSEC-2024-0417` — `gdkx11`
- `RUSTSEC-2024-0418` — `gdk-sys`
- `RUSTSEC-2024-0419` — `gtk3-macros`
- `RUSTSEC-2024-0420` — `gtk-sys`
- `RUSTSEC-2024-0370` — `proc-macro-error`, through `gtk3-macros`
- `RUSTSEC-2024-0429` — `glib`; the affected `VariantStrIter` is part of the Linux GTK graph

FamVoice is explicitly Windows-only, so these packages are not built into the supported application target. They remain in `Cargo.lock` because Cargo records target-conditional dependencies.

## Cross-target transitive maintenance warnings

These six unmaintained crates are pulled by Tauri's parsing graphs: `fxhash` through `tauri-utils -> kuchikiki -> selectors`, and the `unic-*` crates through `tauri-utils -> urlpattern`:

- `RUSTSEC-2025-0057` — `fxhash`
- `RUSTSEC-2025-0075` — `unic-char-range`
- `RUSTSEC-2025-0080` — `unic-common`
- `RUSTSEC-2025-0081` — `unic-char-property`
- `RUSTSEC-2025-0098` — `unic-ucd-version`
- `RUSTSEC-2025-0100` — `unic-ucd-ident`

These are maintenance warnings, not reported vulnerabilities. There is no compatible direct FamVoice update that removes them while the current Tauri resolution still uses this parser graph. Recheck them whenever Tauri is updated.

## Cross-target `rand` soundness advisory

`RUSTSEC-2026-0097` affects `rand 0.7.3`, reached here only through the build-time chain `tauri-utils -> kuchikiki -> selectors -> phf_codegen -> phf_generator`. Exploitation requires all of the advisory's custom-logger and re-entrant `thread_rng` conditions. FamVoice defines no custom Rust logger and does not call this build dependency at runtime, so the condition is not reachable in the supported app.

This is a documented tolerable risk, not a blanket ignore. Reassess if FamVoice adds a custom Rust logger, directly uses the affected `rand` API, or Tauri updates the chain.

## Revalidation commands

```powershell
cargo audit --file src-tauri/Cargo.lock
cargo tree --manifest-path src-tauri/Cargo.toml --target x86_64-pc-windows-msvc -i glib@0.18.5
cargo tree --manifest-path src-tauri/Cargo.toml --target x86_64-pc-windows-msvc -i rand@0.7.3
cargo tree --manifest-path src-tauri/Cargo.toml --target x86_64-pc-windows-msvc -i fxhash@0.2.1
```
