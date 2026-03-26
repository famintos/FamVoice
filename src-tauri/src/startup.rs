use std::env::current_exe;
use std::path::{Component, Path};

use tauri::AppHandle;
use tauri_plugin_autostart::ManagerExt as _;

const UNSAFE_AUTOSTART_PATHS: &[&[&str]] = &[
    &["src-tauri", "target", "debug"],
    &["src-tauri", "target", "release"],
];

fn normalized_components(path: &Path) -> Vec<String> {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(part) => Some(part.to_string_lossy().to_ascii_lowercase()),
            _ => None,
        })
        .collect()
}

fn has_component_sequence(components: &[String], sequence: &[&str]) -> bool {
    if components.len() < sequence.len() {
        return false;
    }

    let normalized_sequence = sequence
        .iter()
        .map(|part| part.to_ascii_lowercase())
        .collect::<Vec<_>>();

    components
        .windows(normalized_sequence.len())
        .any(|window| window == normalized_sequence.as_slice())
}

pub(crate) fn executable_supports_autostart(path: &Path) -> bool {
    let components = normalized_components(path);

    !UNSAFE_AUTOSTART_PATHS
        .iter()
        .any(|sequence| has_component_sequence(&components, sequence))
}

pub(crate) fn current_executable_supports_autostart() -> bool {
    current_exe()
        .map(|path| executable_supports_autostart(&path))
        .unwrap_or(true)
}

pub(crate) fn disable_unsafe_autostart_entry(app: &AppHandle) {
    let Ok(exe_path) = current_exe() else {
        return;
    };

    if executable_supports_autostart(&exe_path) {
        return;
    }

    let autolaunch = app.autolaunch();
    match autolaunch.is_enabled() {
        Ok(true) => match autolaunch.disable() {
            Ok(()) => {
                eprintln!(
                    "[FamVoice] Disabled Launch on Startup for development build at {}",
                    exe_path.display()
                );
            }
            Err(error) => {
                eprintln!(
                    "[FamVoice] Failed to disable Launch on Startup for development build at {}: {}",
                    exe_path.display(),
                    error
                );
            }
        },
        Ok(false) => {}
        Err(error) => {
            eprintln!(
                "[FamVoice] Failed to read Launch on Startup state for {}: {}",
                exe_path.display(),
                error
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_dev_executable_does_not_support_autostart() {
        assert!(!executable_supports_autostart(Path::new(
            r"C:\Users\henri\Desktop\app_test\FamVoice\src-tauri\target\debug\famvoice.exe"
        )));
    }

    #[test]
    fn test_release_target_executable_does_not_support_autostart() {
        assert!(!executable_supports_autostart(Path::new(
            r"C:\Users\henri\Desktop\app_test\FamVoice\src-tauri\target\release\famvoice.exe"
        )));
    }

    #[test]
    fn test_installed_executable_supports_autostart() {
        assert!(executable_supports_autostart(Path::new(
            r"C:\Users\henri\AppData\Local\Programs\FamVoice\FamVoice.exe"
        )));
    }
}
