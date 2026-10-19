//! A TSF key-down to the shell's `KeyInput`, by physical position only (`shanjie_proto::classify`), as KeyMap.swift
//! reads the key code and never the character: the keyboard layout cannot change what the core receives.
use shanjie_proto::{classify, Class, RawKey, Special};
use shanjie_shell::keys::{self, KeyInput};
use shanjie_shell::Modifier;

/// A modifier key alone for `Shell::modifier` (單按 Shift): Shift by side (the scan code, as VK_SHIFT does not say),
/// Control, Alt and the Windows keys as `Other`; `None` for Caps Lock, Num Lock, Scroll Lock and every other key.
pub fn modifier(k: RawKey) -> Option<Modifier> {
    match k.vk {
        0x10 => Some(Modifier::Shift { right: k.scan == 0x36 }),
        0xA0 => Some(Modifier::Shift { right: false }),
        0xA1 => Some(Modifier::Shift { right: true }),
        0x11 | 0x12 | 0x5B | 0x5C | 0xA2..=0xA5 => Some(Modifier::Other),
        _ => None,
    }
}

/// `None`: a modifier key alone, which is no key-down for the shell (macOS: flagsChanged).
pub fn translate(k: RawKey) -> Option<KeyInput> {
    // The protocol's modifier bits are the shell's (SHIFT, CONTROL, OPTION=Alt, COMMAND=Windows key, CAPSLOCK).
    let m = k.modifiers as u32 & 0b11111;
    Some(match classify(k) {
        Class::Modifier => return None,
        Class::Keycap(c) => KeyInput::Mapped(keys::char_key(c, m)),
        Class::Special(s) => KeyInput::Mapped(keys::special(
            match s {
                Special::Enter => keys::ENTER,
                Special::Space => keys::SPACE,
                Special::Backspace => keys::BACKSPACE,
                Special::Delete => keys::DELETE,
                Special::Escape => keys::ESC,
                Special::Left => keys::LEFT,
                Special::Right => keys::RIGHT,
                Special::Up => keys::UP,
                Special::Down => keys::DOWN,
                Special::Home => keys::HOME,
                Special::End => keys::END,
                Special::Tab => keys::TAB,
            },
            m,
        )),
        Class::Page { up } => KeyInput::Page { up, modifiers: m },
        Class::Other => KeyInput::Unmapped,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keycaps_are_keymap_swifts() {
        for c in shanjie_proto::keycaps() {
            assert!(keys::ANSI_KEYCAPS.contains(c), "{c}");
        }
        assert_eq!(shanjie_proto::keycaps().count(), keys::ANSI_KEYCAPS.chars().count());
    }

    #[test]
    fn lone_modifiers_by_side() {
        let k = |vk, scan| modifier(RawKey { vk, scan, extended: false, modifiers: 0 });
        assert_eq!(k(0x10, 0x2A), Some(Modifier::Shift { right: false }));
        assert_eq!(k(0x10, 0x36), Some(Modifier::Shift { right: true }));
        assert_eq!(k(0x11, 0x1D), Some(Modifier::Other));
        assert_eq!(k(0x14, 0x3A), None, "Caps Lock");
        assert_eq!(k(0x41, 0x1E), None);
    }

    #[test]
    fn modifier_bits_line_up() {
        let k = translate(RawKey { vk: 0x41, scan: 0x1E, extended: false, modifiers: shanjie_proto::MOD_ALT | shanjie_proto::MOD_WIN });
        match k {
            Some(KeyInput::Mapped(k)) => assert_eq!(k.modifiers, keys::OPTION | keys::COMMAND),
            _ => panic!(),
        }
    }
}
