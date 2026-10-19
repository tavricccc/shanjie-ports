//! IBus key events to the shell's `KeyInput`, as macos/Sources/ShanjieKit/KeyMap.swift does it on macOS: only the physical
//! key (the evdev code IBus passes as `keycode`) and the modifier state are read, never the keyval's character, so the
//! desktop's keyboard layout cannot change what the core receives.
//!
//! KeyMap.swift's tables, by physical position:
//! - specials: Return and keypad Enter, Space, Delete (Backspace), Forward Delete, Escape, the four arrows, Home, End, Tab;
//! - the ANSI keycaps a-z, 0-9 and `` ` - = [ ] \ ; ' , . / ``;
//! - Page Up / Page Down handed to the shell, which sends them only while the vertical window is open.
//! Everything else (keypad digits, function keys, Insert, ...) is outside the tables.
//!
//! Modifier keys pressed on their own are not key-down events on macOS (they are flagsChanged, which the shell ignores):
//! here they are `None`, so the platform passes them without touching the composition.
use shanjie_shell::keys::{self, KeyInput};
use shanjie_shell::Modifier;

// IBusModifierType (ibustypes.h).
pub const SHIFT_MASK: u32 = 1 << 0;
pub const LOCK_MASK: u32 = 1 << 1;
pub const CONTROL_MASK: u32 = 1 << 2;
pub const MOD1_MASK: u32 = 1 << 3;
pub const MOD4_MASK: u32 = 1 << 6;
pub const SUPER_MASK: u32 = 1 << 26;
pub const RELEASE_MASK: u32 = 1 << 30;

/// bit0 SHIFT, bit1 CONTROL, bit2 OPTION (Alt, the key in the Option position), bit3 COMMAND (Super, the key in the Command
/// position), bit4 CAPSLOCK.
pub fn modifiers(state: u32) -> u32 {
    let mut m = 0;
    if state & SHIFT_MASK != 0 {
        m |= keys::SHIFT;
    }
    if state & CONTROL_MASK != 0 {
        m |= keys::CONTROL;
    }
    if state & MOD1_MASK != 0 {
        m |= keys::OPTION;
    }
    if state & (SUPER_MASK | MOD4_MASK) != 0 {
        m |= keys::COMMAND;
    }
    if state & LOCK_MASK != 0 {
        m |= keys::CAPSLOCK;
    }
    m
}

/// Keyvals of keys that only change modifiers (ibuskeysyms.h): Shift, Control, Caps Lock, Shift Lock, Meta, Alt, Super,
/// Hyper, ISO Level 3/5 shift and lock, Num Lock, Scroll Lock, Mode switch.
fn modifier_only(keyval: u32) -> bool {
    matches!(keyval, 0xffe1..=0xffee | 0xfe01..=0xfe13 | 0xff7f | 0xff14 | 0xff7e)
}

/// A modifier key pressed (true) or released on its own, for the Windows habit 單按 Shift (`Shell::modifier`): Shift by
/// side, Control, Alt, Meta and Super as `Other`. `None` for every other key.
pub fn modifier(keyval: u32, state: u32) -> Option<(Modifier, bool)> {
    let m = match keyval {
        0xffe1 => Modifier::Shift { right: false },
        0xffe2 => Modifier::Shift { right: true },
        0xffe3..=0xffe4 | 0xffe7..=0xffee => Modifier::Other,
        _ => return None,
    };
    Some((m, state & RELEASE_MASK == 0))
}

/// evdev codes (linux/input-event-codes.h).
fn special(code: u32) -> Option<u32> {
    Some(match code {
        28 | 96 => keys::ENTER, // KEY_ENTER, KEY_KPENTER
        57 => keys::SPACE,
        14 => keys::BACKSPACE,
        111 => keys::DELETE,
        1 => keys::ESC,
        105 => keys::LEFT,
        106 => keys::RIGHT,
        103 => keys::UP,
        108 => keys::DOWN,
        102 => keys::HOME,
        107 => keys::END,
        15 => keys::TAB,
        _ => return None,
    })
}

/// The unshifted ANSI keycap of an evdev code.
fn keycap(code: u32) -> Option<char> {
    const ROW: [(u32, char); 47] = [
        (2, '1'), (3, '2'), (4, '3'), (5, '4'), (6, '5'), (7, '6'), (8, '7'), (9, '8'), (10, '9'), (11, '0'),
        (12, '-'), (13, '='), (16, 'q'), (17, 'w'), (18, 'e'), (19, 'r'), (20, 't'), (21, 'y'), (22, 'u'),
        (23, 'i'), (24, 'o'), (25, 'p'), (26, '['), (27, ']'), (30, 'a'), (31, 's'), (32, 'd'), (33, 'f'),
        (34, 'g'), (35, 'h'), (36, 'j'), (37, 'k'), (38, 'l'), (39, ';'), (40, '\''), (41, '`'), (43, '\\'),
        (44, 'z'), (45, 'x'), (46, 'c'), (47, 'v'), (48, 'b'), (49, 'n'), (50, 'm'), (51, ','), (52, '.'),
        (53, '/'),
    ];
    ROW.iter().find(|(c, _)| *c == code).map(|(_, ch)| *ch)
}

/// `None`: not a key-down the shell sees at all (a release, or a modifier key alone).
pub fn translate(keyval: u32, keycode: u32, state: u32) -> Option<KeyInput> {
    if state & RELEASE_MASK != 0 || modifier_only(keyval) {
        return None;
    }
    let m = modifiers(state);
    Some(match keycode {
        104 => KeyInput::Page { up: true, modifiers: m },
        109 => KeyInput::Page { up: false, modifiers: m },
        _ => match special(keycode) {
            Some(kind) => KeyInput::Mapped(keys::special(kind, m)),
            None => match keycap(keycode) {
                Some(ch) => KeyInput::Mapped(keys::char_key(ch, m)),
                None => KeyInput::Unmapped,
            },
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mapped(k: Option<KeyInput>) -> (u32, u32, u32) {
        match k {
            Some(KeyInput::Mapped(k)) => (k.kind, k.ch, k.modifiers),
            _ => panic!("not a mapped key"),
        }
    }

    #[test]
    fn letters_follow_the_physical_key() {
        assert_eq!(mapped(translate('A' as u32, 30, SHIFT_MASK)), (keys::CHAR, 'a' as u32, keys::SHIFT));
        // Dvorak's apostrophe sits on the q position: the core still gets q.
        assert_eq!(mapped(translate('\'' as u32, 16, 0)).1, 'q' as u32);
    }

    #[test]
    fn every_keycap_is_in_keymap_swift() {
        for code in 0..256 {
            if let Some(c) = keycap(code) {
                assert!(shanjie_shell::keys::ANSI_KEYCAPS.contains(c));
            }
        }
        assert_eq!((0..256).filter_map(keycap).count(), shanjie_shell::keys::ANSI_KEYCAPS.chars().count());
    }

    #[test]
    fn keypad_digits_and_function_keys_are_outside_the_tables() {
        assert!(matches!(translate(0xffb1, 79, 0), Some(KeyInput::Unmapped)));
        assert!(matches!(translate(0xffbe, 59, 0), Some(KeyInput::Unmapped)));
        assert_eq!(mapped(translate(0xff8d, 96, 0)).0, keys::ENTER, "keypad Enter is Enter (KeyMap.swift 76)");
    }

    #[test]
    fn modifier_keys_and_releases_are_not_key_downs() {
        assert!(translate(0xffe1, 42, 0).is_none());
        assert!(translate(0xffe5, 58, 0).is_none(), "Caps Lock");
        assert!(translate('a' as u32, 30, RELEASE_MASK).is_none());
    }

    #[test]
    fn lone_modifiers_for_the_shift_tap() {
        assert_eq!(modifier(0xffe1, 0), Some((Modifier::Shift { right: false }, true)));
        assert_eq!(modifier(0xffe2, RELEASE_MASK | SHIFT_MASK), Some((Modifier::Shift { right: true }, false)));
        assert_eq!(modifier(0xffe3, 0), Some((Modifier::Other, true)));
        assert_eq!(modifier('a' as u32, 0), None);
        assert_eq!(modifier(0xffe5, 0), None, "Caps Lock is not part of a tap");
    }

    #[test]
    fn page_keys_and_modifier_bits() {
        assert!(matches!(translate(0xff56, 109, 0), Some(KeyInput::Page { up: false, modifiers: 0 })));
        assert_eq!(modifiers(SHIFT_MASK | CONTROL_MASK | MOD1_MASK | LOCK_MASK | SUPER_MASK), 0b11111);
    }
}
