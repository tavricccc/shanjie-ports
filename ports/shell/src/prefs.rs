//! The stored settings: the macOS app's UserDefaults keys (macos/Sources/Shanjie/main.swift, the `Defaults*Store`
//! classes), kept in a small text file instead, and the values a key means while it was never chosen.
//!
//! Every key has the same name and the same "absent means" value as on macOS. A port changes none of them: the values
//! a never-chosen key means can only come from the packaged `defaults.ini` (`Defaults::load`), which the package
//! scripts write from `ports/overlay/` and which is empty while the port follows upstream.
//!
//! File format, one `key=value` per line, `#` comments, UTF-8: booleans `true`/`false`, numbers in Rust's `f64`
//! syntax, strings as they are (no newline or `=` in keys). `appProfiles` is one line per app: `appProfiles.<id>=chat`.
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The keys of main.swift, as stored.
pub mod key {
    pub const LAYOUT: &str = "layout";
    pub const DEMOTE: &str = "demoteSensitive";
    pub const PREDICTION: &str = "prediction";
    pub const ABBREVIATION: &str = "abbreviation";
    pub const ENGLISH: &str = "english";
    pub const ENGLISH_INLINE: &str = "englishInline";
    pub const CJK_SPACING: &str = "cjkSpacing";
    pub const ACG_PACK: &str = "acgPack";
    pub const GLASS_TINT: &str = "glassTint";
    pub const CANDIDATE_VERTICAL: &str = "candidateVertical";
    pub const RERANK: &str = "rerank";
    pub const RERANK_LEVEL: &str = "rerankLevel";
    pub const PROFILE_MODE: &str = "profileMode";
    /// Prefix of the per-app choices (`appProfiles` dictionary on macOS).
    pub const APP_PROFILES: &str = "appProfiles.";
}

/// What a never-chosen key means. `upstream()` is what the macOS shell uses (Shell.swift's `?? value` at each read).
#[derive(Clone, Debug, PartialEq)]
pub struct Defaults {
    pub layout: String,
    pub demote: bool,
    pub prediction: bool,
    pub abbreviation: bool,
    pub english: bool,
    pub english_inline: bool,
    pub cjk_spacing: bool,
    pub acg_pack: bool,
    pub glass_tint: f64,
    pub candidate_vertical: bool,
    pub rerank: bool,
    pub rerank_level: String,
    pub profile_mode: String,
    /// The Windows habits' keys (`win.*`, winops.rs) the overlay sets, as written; absent: upstream's behavior.
    pub win: BTreeMap<String, String>,
}

impl Defaults {
    pub fn upstream() -> Defaults {
        Defaults {
            layout: "standard".into(),
            demote: true,
            prediction: true,
            abbreviation: false,
            english: true,
            english_inline: true,
            cjk_spacing: false,
            acg_pack: true,
            glass_tint: 0.0,
            candidate_vertical: false,
            rerank: true,
            rerank_level: "standard".into(),
            profile_mode: "auto".into(),
            win: BTreeMap::new(),
        }
    }

    /// The upstream values, then whatever `defaults.ini` sets (the overlay). A missing file is the upstream values; an
    /// unreadable line or an unknown key is skipped and reported in the returned list (the caller logs it, never a value).
    pub fn load(path: &Path) -> (Defaults, Vec<String>) {
        let mut d = Defaults::upstream();
        let mut problems = Vec::new();
        let Ok(text) = std::fs::read_to_string(path) else { return (d, problems) };
        for (n, (k, v)) in parse(&text).into_iter().enumerate() {
            let ok = match k.as_str() {
                key::LAYOUT => set_choice(&mut d.layout, &v, &["standard", "eten"]),
                key::DEMOTE => set_bool(&mut d.demote, &v),
                key::PREDICTION => set_bool(&mut d.prediction, &v),
                key::ABBREVIATION => set_bool(&mut d.abbreviation, &v),
                key::ENGLISH => set_bool(&mut d.english, &v),
                key::ENGLISH_INLINE => set_bool(&mut d.english_inline, &v),
                key::CJK_SPACING => set_bool(&mut d.cjk_spacing, &v),
                key::ACG_PACK => set_bool(&mut d.acg_pack, &v),
                key::GLASS_TINT => v.parse::<f64>().ok().filter(|x| (0.0..=1.0).contains(x)).map(|x| d.glass_tint = x).is_some(),
                key::CANDIDATE_VERTICAL => set_bool(&mut d.candidate_vertical, &v),
                key::RERANK => set_bool(&mut d.rerank, &v),
                key::RERANK_LEVEL => set_choice(&mut d.rerank_level, &v, &["conservative", "standard", "aggressive"]),
                key::PROFILE_MODE => set_choice(&mut d.profile_mode, &v, &["auto", "chat", "formal"]),
                k if crate::winops::valid_default(k, &v) => {
                    d.win.insert(k.to_string(), v.clone());
                    true
                }
                _ => false,
            };
            if !ok {
                problems.push(format!("defaults.ini entry {} ({k}) was not applied", n + 1));
            }
        }
        (d, problems)
    }
}

fn set_bool(slot: &mut bool, v: &str) -> bool {
    match parse_bool(v) {
        Some(b) => {
            *slot = b;
            true
        }
        None => false,
    }
}

fn set_choice(slot: &mut String, v: &str, allowed: &[&str]) -> bool {
    if allowed.contains(&v) {
        *slot = v.to_string();
        true
    } else {
        false
    }
}

pub(crate) fn parse_bool(v: &str) -> Option<bool> {
    match v {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

/// `key=value` lines in file order; blank lines and `#` comments skipped; whitespace around both trimmed.
pub fn parse(text: &str) -> Vec<(String, String)> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|l| l.split_once('=').map(|(k, v)| (k.trim().to_string(), v.trim().to_string())))
        .filter(|(k, _)| !k.is_empty())
        .collect()
}

/// The user's stored settings: UserDefaults' role. `None` (absent) means never chosen.
pub struct Prefs {
    path: Option<PathBuf>,
    values: BTreeMap<String, String>,
}

impl Prefs {
    /// In memory only, as the Swift tests' `Memory*Store`s: nothing is ever read or written.
    pub fn memory() -> Prefs {
        Prefs { path: None, values: BTreeMap::new() }
    }

    /// The file at `path`; missing or unreadable is empty (the app then starts as never configured, like an empty
    /// UserDefaults domain), and the first change writes it.
    pub fn open(path: PathBuf) -> Prefs {
        let values = std::fs::read_to_string(&path).map(|t| parse(&t).into_iter().collect()).unwrap_or_default();
        Prefs { path: Some(path), values }
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    pub fn string(&self, k: &str) -> Option<String> {
        self.values.get(k).cloned()
    }

    pub fn bool(&self, k: &str) -> Option<bool> {
        self.values.get(k).and_then(|v| parse_bool(v))
    }

    pub fn double(&self, k: &str) -> Option<f64> {
        self.values.get(k).and_then(|v| v.parse().ok())
    }

    pub fn set_string(&mut self, k: &str, v: &str) {
        self.values.insert(k.to_string(), v.to_string());
        self.save();
    }

    pub fn set_bool(&mut self, k: &str, v: bool) {
        self.set_string(k, if v { "true" } else { "false" });
    }

    pub fn set_double(&mut self, k: &str, v: f64) {
        self.set_string(k, &v.to_string());
    }

    /// The raw per-app entries (`appProfiles.<id>` = value), unfiltered: `AppProfiles::parse` filters them.
    pub fn app_profiles(&self) -> Vec<(String, String)> {
        self.values
            .iter()
            .filter_map(|(k, v)| k.strip_prefix(key::APP_PROFILES).map(|id| (id.to_string(), v.clone())))
            .collect()
    }

    /// Replaces every per-app entry (an empty map removes them all, as `removeObject(forKey: "appProfiles")`).
    pub fn set_app_profiles(&mut self, profiles: &BTreeMap<String, String>) {
        self.values.retain(|k, _| !k.starts_with(key::APP_PROFILES));
        for (id, v) in profiles {
            if valid_id(id) {
                self.values.insert(format!("{}{id}", key::APP_PROFILES), v.clone());
            }
        }
        self.save();
    }

    /// Atomic: a temporary file next to it, then a rename. A failure keeps the values in memory (as UserDefaults does
    /// while its own write is pending) and is not reported: nothing about the settings is ever logged.
    fn save(&self) {
        let Some(path) = &self.path else { return };
        let mut text = String::from("# Shanjie settings (the macOS app's UserDefaults keys). Written by the input method.\n");
        for (k, v) in &self.values {
            text.push_str(k);
            text.push('=');
            text.push_str(v);
            text.push('\n');
        }
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let tmp = path.with_extension("tmp");
        if std::fs::write(&tmp, text).is_ok() && std::fs::rename(&tmp, path).is_err() {
            let _ = std::fs::remove_file(&tmp);
        }
    }
}

/// An app identifier that can be a key: no line break, `=` or `#` at the start (the file format), not empty.
pub fn valid_id(id: &str) -> bool {
    !id.is_empty() && !id.starts_with('#') && !id.contains(['\n', '\r', '=']) && id.trim() == id
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upstream_defaults_match_shell_swift() {
        let d = Defaults::upstream();
        assert!(d.demote && d.prediction && d.english && d.english_inline && d.acg_pack && d.rerank);
        assert!(!d.abbreviation && !d.candidate_vertical);
        assert_eq!((d.layout.as_str(), d.rerank_level.as_str(), d.profile_mode.as_str()), ("standard", "standard", "auto"));
        assert_eq!(d.glass_tint, 0.0);
    }

    #[test]
    fn overlay_file_overrides_and_reports_bad_entries() {
        let dir = std::env::temp_dir().join(format!("shanjie-defaults-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("defaults.ini");
        std::fs::write(&p, "# overlay\nprediction=false\nrerankLevel=aggressive\nlayout=dvorak\nunknown=1\n").unwrap();
        let (d, problems) = Defaults::load(&p);
        assert!(!d.prediction);
        assert_eq!(d.rerank_level, "aggressive");
        assert_eq!(d.layout, "standard", "an unknown layout is not applied");
        assert_eq!(problems.len(), 2);
        let (none, quiet) = Defaults::load(&dir.join("missing.ini"));
        assert_eq!(none, Defaults::upstream());
        assert!(quiet.is_empty());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn prefs_round_trip() {
        let dir = std::env::temp_dir().join(format!("shanjie-prefs-{}", std::process::id()));
        let p = dir.join("settings.ini");
        let mut s = Prefs::open(p.clone());
        assert_eq!(s.bool(key::PREDICTION), None);
        s.set_bool(key::PREDICTION, false);
        s.set_string(key::LAYOUT, "eten");
        s.set_app_profiles(&[("discord".to_string(), "formal".to_string())].into_iter().collect());
        let t = Prefs::open(p);
        assert_eq!(t.bool(key::PREDICTION), Some(false));
        assert_eq!(t.string(key::LAYOUT).as_deref(), Some("eten"));
        assert_eq!(t.app_profiles(), vec![("discord".to_string(), "formal".to_string())]);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
