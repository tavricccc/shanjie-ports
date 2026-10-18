//! shanjie-host.exe. UNVERIFIED ON WINDOWS (cross-compiled only; ports/windows/README.md).
#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(windows)]
mod win;

#[cfg(windows)]
fn main() {
    win::run();
}

#[cfg(not(windows))]
fn main() {
    eprintln!("shanjie-host runs on Windows only; its logic is tested with `cargo test -p shanjie-host`");
    std::process::exit(2);
}
