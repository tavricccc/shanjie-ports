//! The platform-neutral part of upstream's macOS shell (macos/Sources/ShanjieKit), ported to Rust so the Linux and
//! Windows ports run the core exactly as the macOS app does: the same core calls in the same order, the same settings
//! with the same defaults, the same menu. A platform supplies only what ShanjieKit gets from AppKit and IMK: the keys,
//! the client's text and identity, and somewhere to draw (ports/docs/porting.md).
pub mod core;
pub mod keys;
pub mod menu;
pub mod menu_tree;
pub mod panel;
pub mod prefs;
pub mod shell;
pub mod spacing;

pub use shell::{ClientInfo, Effect, Effects, LeftText, PlatformTables, RerankRunner, SessionId, Setup, Shell};
