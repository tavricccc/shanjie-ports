//! Port of macos/Sources/ShanjieKit/CJKSpacing.swift (docs/contracts/cjk-spacing.md section 1): a space between a Han
//! character and an ASCII letter or digit, applied only to the text the shell commits. Never logged.
//!
//! Swift walks `Character`s (grapheme clusters). Rust has no grapheme segmentation in std, so a cluster here is a
//! scalar followed by the scalars that extend it and can decide a cluster's kind: variation selectors (a Han character
//! with one is still Han), combining diacritics and the zero-width joiner (an ASCII letter with one is no longer ASCII,
//! as `Character.isASCII` says). Every other cluster is `Other` either way, so the result is the same.
use crate::shell::is_han;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Han,
    Ascii,
    Other,
}

fn extends(c: char) -> bool {
    matches!(c as u32, 0x0300..=0x036F | 0x200D | 0xFE00..=0xFE0F | 0xE0100..=0xE01EF)
}

/// The text's clusters, in order.
fn clusters(text: &str) -> impl Iterator<Item = &str> {
    let mut rest = text;
    std::iter::from_fn(move || {
        let mut chars = rest.char_indices();
        chars.next()?;
        let end = chars.find(|(_, c)| !extends(*c)).map_or(rest.len(), |(i, _)| i);
        let (cluster, tail) = rest.split_at(end);
        rest = tail;
        Some(cluster)
    })
}

fn kind(cluster: &str) -> Kind {
    let mut scalars = cluster.chars();
    match (scalars.next(), scalars.next()) {
        // A-Z a-z 0-9 only.
        (Some(c), None) if c.is_ascii() => {
            if c.is_ascii_alphanumeric() {
                Kind::Ascii
            } else {
                Kind::Other
            }
        }
        // The first scalar, like the core's per-scalar is_han.
        (Some(c), _) if is_han(c) => Kind::Han,
        _ => Kind::Other,
    }
}

/// The kind of the text's last character, `None` for an empty text.
pub fn last(text: &str) -> Option<Kind> {
    clusters(text).last().map(kind)
}

fn needs_space(a: Kind, b: Kind) -> bool {
    matches!((a, b), (Kind::Han, Kind::Ascii) | (Kind::Ascii, Kind::Han))
}

/// `CJKSpacing.apply(previous:text:)`. `previous` is the kind of the character before the insertion point, `None` when
/// unknown (no leading space then).
pub fn apply(previous: Option<Kind>, text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut last = previous.unwrap_or(Kind::Other);
    for c in clusters(text) {
        let k = kind(c);
        if needs_space(last, k) {
            out.push(' ');
        }
        out.push_str(c);
        last = k;
    }
    out
}
