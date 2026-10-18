//! A TSF key-down to the shell's `KeyInput`, by physical position only (`shanjie_proto::classify`), as KeyMap.swift
//! reads the key code and never the character: the keyboard layout cannot change what the core receives.
use shanjie_proto::{classify, Class, RawKey, Special};
use shanjie_shell::keys::{self, KeyInput};

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
    fn modifier_bits_line_up() {
        let k = translate(RawKey { vk: 0x41, scan: 0x1E, extended: false, modifiers: shanjie_proto::MOD_ALT | shanjie_proto::MOD_WIN });
        match k {
            Some(KeyInput::Mapped(k)) => assert_eq!(k.modifiers, keys::OPTION | keys::COMMAND),
            _ => panic!(),
        }
    }
}
