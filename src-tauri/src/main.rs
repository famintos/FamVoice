#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // Safety net: if an inherited console is attached (e.g. from autostart
    // launching via a shell wrapper), detach from it so no CMD window lingers.
    #[cfg(all(windows, not(debug_assertions)))]
    unsafe {
        windows_sys::Win32::System::Console::FreeConsole();
    }

    famvoice_lib::run()
}
