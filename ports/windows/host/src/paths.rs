//! Where the host finds and keeps its files on Windows (the macOS app's places in brackets):
//! - data [Contents/Resources]: `SHANJIE_DATA`, else `data` beside shanjie-host.exe (the versioned install folder);
//! - learning [~/Library/Application Support/shanjie]: `%LOCALAPPDATA%\Shanjie` (the earlier Windows port's folder: the
//!   same learning.tsv format, so its learning carries over);
//! - settings [the app's UserDefaults domain]: `%LOCALAPPDATA%\Shanjie\settings.ini`.
use std::path::PathBuf;

pub fn data() -> PathBuf {
    if let Some(d) = std::env::var_os("SHANJIE_DATA").filter(|d| !d.is_empty()) {
        return PathBuf::from(d);
    }
    let exe = std::env::current_exe().unwrap_or_default();
    exe.parent().map(|p| p.join("data")).unwrap_or_default()
}

fn local() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA").filter(|v| !v.is_empty()).map(PathBuf::from).filter(|p| p.is_absolute())
}

pub fn learning() -> Option<PathBuf> {
    local().map(|d| d.join("Shanjie"))
}

pub fn settings() -> Option<PathBuf> {
    local().map(|d| d.join("Shanjie").join("settings.ini"))
}
