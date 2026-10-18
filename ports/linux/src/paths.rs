//! Where the files go on Linux (the macOS app's places in brackets):
//! - data folder [Contents/Resources]: `SHANJIE_DATA`, else `<exe>/../../share/shanjie/data` (the installed layout);
//! - learning [~/Library/Application Support/shanjie]: `$XDG_DATA_HOME/shanjie`, default `~/.local/share/shanjie`;
//! - settings [the app's UserDefaults domain]: `$XDG_CONFIG_HOME/shanjie/settings.ini`, default `~/.config/shanjie`.
use std::path::PathBuf;

fn home() -> Option<PathBuf> {
    std::env::var_os("HOME").filter(|h| !h.is_empty()).map(PathBuf::from)
}

fn xdg(var: &str, fallback: &str) -> Option<PathBuf> {
    std::env::var_os(var)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| home().map(|h| h.join(fallback)))
}

pub fn data() -> PathBuf {
    if let Some(d) = std::env::var_os("SHANJIE_DATA").filter(|d| !d.is_empty()) {
        return PathBuf::from(d);
    }
    let exe = std::env::current_exe().unwrap_or_default();
    exe.parent().and_then(|p| p.parent()).map(|p| p.join("share/shanjie/data")).unwrap_or_default()
}

pub fn learning() -> Option<PathBuf> {
    xdg("XDG_DATA_HOME", ".local/share").map(|d| d.join("shanjie"))
}

pub fn settings() -> Option<PathBuf> {
    xdg("XDG_CONFIG_HOME", ".config").map(|d| d.join("shanjie/settings.ini"))
}
