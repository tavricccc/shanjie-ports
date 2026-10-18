//! The core's key kinds and modifier bits (shanjie.h; macos/Sources/ShanjieKit/KeyMap.swift), and what a platform hands
//! the shell for one key event.
//!
//! Each platform builds `KeyInput` from the physical key, never from the typed character, as KeyMap.swift does: the
//! special keys by their kind, every other key as its unshifted keycap on the ANSI layout, keys outside both tables as
//! `Unmapped`. Page Up / Page Down are `Page`: the shell decides whether they reach the core (KeyMap.swift `pageKeys`).
pub use shanjie_core::ffi::ShanjieKey;

pub const CHAR: u32 = 1;
pub const SPACE: u32 = 2;
pub const ENTER: u32 = 3;
pub const BACKSPACE: u32 = 4;
pub const DELETE: u32 = 5;
pub const ESC: u32 = 6;
pub const LEFT: u32 = 7;
pub const RIGHT: u32 = 8;
pub const UP: u32 = 9;
pub const DOWN: u32 = 10;
pub const HOME: u32 = 11;
pub const END: u32 = 12;
pub const TAB: u32 = 13;
pub const PAGE_UP: u32 = 14;
pub const PAGE_DOWN: u32 = 15;

/// bit0 SHIFT, bit1 CONTROL, bit2 OPTION, bit3 COMMAND, bit4 CAPSLOCK.
pub const SHIFT: u32 = 1 << 0;
pub const CONTROL: u32 = 1 << 1;
pub const OPTION: u32 = 1 << 2;
pub const COMMAND: u32 = 1 << 3;
pub const CAPSLOCK: u32 = 1 << 4;

/// One key-down as the platform translated it.
#[derive(Clone, Copy)]
pub enum KeyInput {
    /// In KeyMap.swift's tables.
    Mapped(ShanjieKey),
    /// Page Up (`up` true) or Page Down, with the modifier bits held.
    Page { up: bool, modifiers: u32 },
    /// Outside both tables (function keys, keypad digits, ...): never sent to the core.
    Unmapped,
}

/// The unshifted ANSI keycaps KeyMap.swift's `ansi` table maps, in one string: a platform's table of physical keys must
/// produce only these characters.
pub const ANSI_KEYCAPS: &str = "abcdefghijklmnopqrstuvwxyz0123456789`-=[]\\;',./";

pub fn char_key(ch: char, modifiers: u32) -> ShanjieKey {
    ShanjieKey { kind: CHAR, ch: ch as u32, modifiers }
}

pub fn special(kind: u32, modifiers: u32) -> ShanjieKey {
    ShanjieKey { kind, ch: 0, modifiers }
}
