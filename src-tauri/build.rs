fn main() {
    // `include_image!` is a proc macro: it reads and decodes these PNGs at expansion
    // time, so rustc never records them as source dependencies. Without declaring them
    // here, editing a tray frame leaves the previous one baked into the binary and the
    // change silently does not appear — rebuilding is not enough, because Cargo sees no
    // reason to run one.
    let icons = std::fs::read_dir("icons").expect("src-tauri/icons must exist");

    for entry in icons {
        let path = entry.expect("icons directory must be readable").path();
        let is_tray_frame = path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("tray-") && name.ends_with(".png"));

        if is_tray_frame {
            println!("cargo:rerun-if-changed={}", path.display());
        }
    }

    tauri_build::build()
}
