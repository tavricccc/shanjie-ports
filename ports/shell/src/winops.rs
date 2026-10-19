//! The Windows habits: the old port's settings (shanjie-multiplatform 0.10.2, `profiles/windows.ini`) that the shell can
//! give without changing the core, for people who come from Microsoft Bopomofo or from the old port. Upstream has none of
//! them; every one is off, or at upstream's behavior, until chosen (ports/docs/windows-habits.md).
//!
//! They are the shell's own settings, stored next to the upstream keys with a `win.` prefix, and they reach the core only
//! as the keys it already takes: a habit rewrites, holds back or answers a key before the core sees it, or changes the
//! text the shell commits. The menu shows them as one submenu (the ports have no settings window), the per-key
//! punctuation switches as a second one.
use crate::keys::{self, ShanjieKey};

/// One on/off habit: the stored key, the menu label, and what it means while never chosen (upstream's behavior).
#[derive(Clone, Copy, Debug)]
pub struct Flag {
    pub key: &'static str,
    pub label: &'static str,
    pub upstream: bool,
    /// The old port's Windows template (`profiles/windows.ini`).
    pub windows: bool,
}

pub mod key {
    pub const SHIFT_TOGGLE: &str = "win.shiftToggle";
    pub const SHIFT_SIDE: &str = "win.shiftSide";
    pub const CTRL_SPACE: &str = "win.ctrlSpaceToggle";
    pub const SHIFT_SPACE: &str = "win.shiftSpaceWidth";
    pub const FULL_WIDTH: &str = "win.fullWidth";
    pub const CAPS_ENGLISH: &str = "win.capsLockEnglish";
    pub const CTRL_PUNCT: &str = "win.ctrlPunctuation";
    pub const SYMBOLS: &str = "win.symbolTable";
    pub const CODE_INPUT: &str = "win.codeInput";
    pub const ENTER_COMMITS: &str = "win.enterCommits";
    pub const LEARNING: &str = "win.learning";
    pub const CONTEXT: &str = "win.leftContext";
    pub const MIXED: &str = "win.mixedInput";
    pub const MIXED_TOLERANCE: &str = "win.mixedTolerance";
    /// Prefix of the punctuation switches: `win.punct.shift.<name>`, `win.punct.plain.<name>`.
    pub const PUNCT: &str = "win.punct.";
}

/// The habits with an on/off switch, in menu order.
pub const FLAGS: [Flag; 13] = [
    Flag { key: key::SHIFT_TOGGLE, label: "單按 Shift 切換中／英", upstream: false, windows: true },
    Flag { key: key::CTRL_SPACE, label: "Ctrl＋空白 切換中／英", upstream: false, windows: false },
    Flag { key: key::SHIFT_SPACE, label: "Shift＋空白 切換全形／半形", upstream: false, windows: true },
    Flag { key: key::CAPS_ENGLISH, label: "Caps Lock 打英文", upstream: true, windows: false },
    Flag { key: key::CTRL_PUNCT, label: "Ctrl＋標點 打中文標點", upstream: false, windows: true },
    Flag { key: key::SYMBOLS, label: "Ctrl＋Alt＋逗號 開符號表", upstream: false, windows: true },
    Flag { key: key::CODE_INPUT, label: "反引號＋U 輸入 Unicode 字碼", upstream: false, windows: true },
    Flag { key: key::ENTER_COMMITS, label: "選字時 Enter 直接送出", upstream: false, windows: true },
    Flag { key: key::LEARNING, label: "學習選字", upstream: true, windows: true },
    Flag { key: key::CONTEXT, label: "看游標前的字選字", upstream: true, windows: true },
    Flag { key: key::MIXED, label: "Windows 注音順序混打", upstream: false, windows: false },
    Flag { key: key::MIXED_TOLERANCE, label: "混打：容許多按一次同一鍵", upstream: false, windows: false },
    Flag { key: key::FULL_WIDTH, label: "全形", upstream: false, windows: false },
];

/// Whether `defaults.ini` may set `k` to `v`: a switch to true/false, the Shift side to one of its values.
pub fn valid_default(k: &str, v: &str) -> bool {
    let switch = FLAGS.iter().any(|f| f.key == k) || punct_switches().iter().any(|(p, _)| p == k);
    (switch && matches!(v, "true" | "false")) || (k == key::SHIFT_SIDE && ShiftSide::from_raw(v).is_some())
}

/// Which Shift a lone press toggles with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShiftSide {
    Both,
    Left,
    Right,
}

impl ShiftSide {
    pub fn raw(self) -> &'static str {
        match self {
            ShiftSide::Both => "both",
            ShiftSide::Left => "left",
            ShiftSide::Right => "right",
        }
    }

    pub fn from_raw(s: &str) -> Option<ShiftSide> {
        match s {
            "both" => Some(ShiftSide::Both),
            "left" => Some(ShiftSide::Left),
            "right" => Some(ShiftSide::Right),
            _ => None,
        }
    }

    pub fn allows(self, right: bool) -> bool {
        match self {
            ShiftSide::Both => true,
            ShiftSide::Left => !right,
            ShiftSide::Right => right,
        }
    }
}

/// The core's punctuation keys (core/src/engine.rs `SHIFT_PUNCT`, `PLAIN_PUNCT`): (keycap, the mark, the stored name).
pub const SHIFT_PUNCT: [(char, char, &str); 13] = [
    (',', '，', "comma"),
    ('.', '。', "period"),
    ('/', '？', "slash"),
    ('1', '！', "1"),
    (';', '：', "semicolon"),
    ('[', '『', "lbracket"),
    (']', '』', "rbracket"),
    ('9', '（', "9"),
    ('0', '）', "0"),
    ('`', '～', "backquote"),
    ('\\', '｜', "backslash"),
    ('\'', '“', "quote"),
    ('=', '＋', "equal"),
];
pub const PLAIN_PUNCT: [(char, char, &str); 6] =
    [('[', '「', "lbracket"), (']', '」', "rbracket"), ('\\', '、', "backslash"), ('\'', '‘', "quote"), ('=', '＝', "equal"), ('`', '·', "backquote")];

pub fn punct_key(shift: bool, name: &str) -> String {
    format!("{}{}.{name}", key::PUNCT, if shift { "shift" } else { "plain" })
}

/// Every punctuation switch: (stored key, menu label).
pub fn punct_switches() -> Vec<(String, String)> {
    let shift = SHIFT_PUNCT.iter().map(|(c, m, n)| (punct_key(true, n), format!("Shift＋{c}　{m}")));
    let plain = PLAIN_PUNCT.iter().map(|(c, m, n)| (punct_key(false, n), format!("{c}　{m}")));
    shift.chain(plain).collect()
}

/// The stored key of the switch for `k` when the core would make punctuation of it; `None` for any other key.
/// `zhuyin_or_tone`: whether the unshifted key is a Zhuyin or tone key in the current layout (Eten uses ' and =), where
/// the core makes no plain punctuation.
pub fn punct_switch_of(k: ShanjieKey, zhuyin_or_tone: bool) -> Option<String> {
    if k.kind != keys::CHAR {
        return None;
    }
    let c = char::from_u32(k.ch)?;
    match k.modifiers {
        keys::SHIFT => SHIFT_PUNCT.iter().find(|p| p.0 == c).map(|p| punct_key(true, p.2)),
        0 if !zhuyin_or_tone => PLAIN_PUNCT.iter().find(|p| p.0 == c).map(|p| punct_key(false, p.2)),
        _ => None,
    }
}

/// Ctrl＋標點: the key the core makes the same mark from. Only marks the core has; a key without one passes as before.
/// Ctrl alone gives the full-width forms of the unshifted key (，。「」、：), Ctrl＋Shift those of the shifted key.
pub fn ctrl_punct(k: ShanjieKey) -> Option<ShanjieKey> {
    if k.kind != keys::CHAR {
        return None;
    }
    let c = char::from_u32(k.ch)?;
    match k.modifiers {
        keys::CONTROL => match c {
            ',' | '.' | ';' => Some(keys::char_key(c, keys::SHIFT)),
            '[' | ']' => Some(keys::char_key(c, 0)),
            _ => None,
        },
        m if m == keys::CONTROL | keys::SHIFT => SHIFT_PUNCT.iter().any(|p| p.0 == c).then(|| keys::char_key(c, keys::SHIFT)),
        _ => None,
    }
}

/// The character a key types on the US layout (the keycaps the platforms map, keys.rs `ANSI_KEYCAPS`); `None` for a key
/// with Control, Option or Command.
pub fn typed_char(k: ShanjieKey) -> Option<char> {
    if k.kind == keys::SPACE {
        return (k.modifiers & !(keys::SHIFT | keys::CAPSLOCK) == 0).then_some(' ');
    }
    if k.kind != keys::CHAR || k.modifiers & !(keys::SHIFT | keys::CAPSLOCK) != 0 {
        return None;
    }
    let c = char::from_u32(k.ch)?;
    let shift = k.modifiers & keys::SHIFT != 0;
    if c.is_ascii_lowercase() {
        let caps = k.modifiers & keys::CAPSLOCK != 0;
        return Some(if shift != caps { c.to_ascii_uppercase() } else { c });
    }
    if !shift {
        return Some(c);
    }
    const PAIRS: [(char, char); 21] = [
        ('1', '!'), ('2', '@'), ('3', '#'), ('4', '$'), ('5', '%'), ('6', '^'), ('7', '&'), ('8', '*'), ('9', '('),
        ('0', ')'), ('-', '_'), ('=', '+'), ('[', '{'), (']', '}'), ('\\', '|'), (';', ':'), ('\'', '"'), (',', '<'),
        ('.', '>'), ('/', '?'), ('`', '~'),
    ];
    PAIRS.iter().find(|p| p.0 == c).map(|p| p.1)
}

/// 全形: ASCII printable characters to their full-width forms, the space to the ideographic space.
pub fn full_width(text: &str) -> String {
    text.chars()
        .map(|c| match c {
            ' ' => '\u{3000}',
            '!'..='~' => char::from_u32(c as u32 + 0xFEE0).unwrap_or(c),
            _ => c,
        })
        .collect()
}

/// The symbol table of Ctrl＋Alt＋逗號 (the old port's `SYMBOLS`), nine to a page.
pub const SYMBOLS: &str = "，。？！；：、（）「」『』【】《》〈〉—…～·＋－×÷＝≠≤≥±∞√％‰℃℉°′″→←↑↓★☆○●□■△▲◎※〒＄€£¥";

/// What the symbol table or the code input shows and what a key did to it.
pub enum SpecialOut {
    /// Shown: the marked text, and the candidates with the selected one (empty for the code input).
    Show { marked: String, candidates: Vec<String>, selected: usize, first: u32, total: u32 },
    /// Done: this text goes to the client (empty: nothing).
    Done(String),
    /// Closed without text, and the key goes on to the core as it would have.
    Pass,
}

/// The symbol table (Ctrl＋Alt＋逗號) and the code input (反引號, then U and up to six hex digits): the old port's
/// `Special`, without the Big5 codes (they need the system's code page 950 table).
#[derive(Clone, Default)]
pub struct Special {
    /// 0 closed, 1 after the backquote, 2 Unicode digits, 4 the symbol table.
    mode: u32,
    digits: String,
    selected: usize,
}

impl Special {
    pub fn active(&self) -> bool {
        self.mode != 0
    }

    pub fn close(&mut self) {
        *self = Special::default();
    }

    pub fn open_symbols(&mut self) -> SpecialOut {
        *self = Special { mode: 4, ..Default::default() };
        self.view()
    }

    pub fn open_code(&mut self) -> SpecialOut {
        *self = Special { mode: 1, ..Default::default() };
        self.view()
    }

    fn view(&self) -> SpecialOut {
        if self.mode == 4 {
            let total = SYMBOLS.chars().count();
            let first = self.selected / 9 * 9;
            let candidates = SYMBOLS.chars().skip(first).take(9).map(String::from).collect();
            return SpecialOut::Show { marked: "符號".into(), candidates, selected: self.selected - first, first: first as u32, total: total as u32 };
        }
        let marked = format!("`{}{}", if self.mode == 2 { "U" } else { "" }, self.digits);
        SpecialOut::Show { marked, candidates: Vec::new(), selected: 0, first: 0, total: 0 }
    }

    /// Item `index` of the page shown.
    pub fn pick(&mut self, index: usize) -> SpecialOut {
        if self.mode != 4 {
            return self.view();
        }
        let first = self.selected / 9 * 9;
        match SYMBOLS.chars().skip(first).take(9).nth(index) {
            Some(c) => {
                self.close();
                SpecialOut::Done(c.to_string())
            }
            None => self.view(),
        }
    }

    pub fn key(&mut self, k: ShanjieKey) -> SpecialOut {
        let plain = k.modifiers & !keys::CAPSLOCK == 0;
        if k.kind == keys::ESC {
            self.close();
            return SpecialOut::Done(String::new());
        }
        if self.mode == 4 {
            let total = SYMBOLS.chars().count();
            if plain {
                match k.kind {
                    keys::CHAR if ('1'..='9').contains(&char::from_u32(k.ch).unwrap_or('\0')) => return self.pick((k.ch - '1' as u32) as usize),
                    keys::ENTER => return self.pick(self.selected % 9),
                    keys::DOWN | keys::RIGHT | keys::TAB => self.selected = (self.selected + 1).min(total - 1),
                    keys::UP | keys::LEFT => self.selected = self.selected.saturating_sub(1),
                    keys::SPACE => {
                        let next = self.selected / 9 * 9 + 9;
                        self.selected = if next < total { next } else { 0 };
                    }
                    keys::BACKSPACE => {
                        self.close();
                        return SpecialOut::Done(String::new());
                    }
                    _ => {}
                }
            }
            return self.view();
        }
        if k.kind == keys::BACKSPACE {
            if !self.digits.is_empty() {
                self.digits.pop();
            } else if self.mode == 2 {
                self.mode = 1;
            } else {
                self.close();
                return SpecialOut::Done(String::new());
            }
            return self.view();
        }
        let c = char::from_u32(k.ch).unwrap_or('\0');
        if self.mode == 1 {
            if k.kind == keys::CHAR && c == 'u' && k.modifiers & !(keys::SHIFT | keys::CAPSLOCK) == 0 {
                self.mode = 2;
                return self.view();
            }
            // Anything else: the backquote was meant as itself (the core's ·), then the key as it would have gone.
            self.close();
            return SpecialOut::Pass;
        }
        if k.kind == keys::CHAR && k.modifiers & !(keys::SHIFT | keys::CAPSLOCK) == 0 && c.is_ascii_hexdigit() {
            if self.digits.len() < 6 {
                self.digits.push(c.to_ascii_uppercase());
            }
            return self.view();
        }
        if (k.kind == keys::SPACE || k.kind == keys::ENTER) && !self.digits.is_empty() {
            let text = u32::from_str_radix(&self.digits, 16).ok().and_then(char::from_u32).filter(|c| *c != '\0');
            if let Some(c) = text {
                self.close();
                return SpecialOut::Done(c.to_string());
            }
        }
        self.view()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_chars_follow_shift_and_caps_lock() {
        assert_eq!(typed_char(keys::char_key('a', 0)), Some('a'));
        assert_eq!(typed_char(keys::char_key('a', keys::SHIFT)), Some('A'));
        assert_eq!(typed_char(keys::char_key('a', keys::CAPSLOCK)), Some('A'));
        assert_eq!(typed_char(keys::char_key('a', keys::SHIFT | keys::CAPSLOCK)), Some('a'));
        assert_eq!(typed_char(keys::char_key('1', keys::SHIFT | keys::CAPSLOCK)), Some('!'));
        assert_eq!(typed_char(keys::char_key('a', keys::CONTROL)), None);
        assert_eq!(full_width("Az1 !~"), "Ａｚ１\u{3000}！～");
    }

    #[test]
    fn punctuation_switches_cover_the_core_tables_once() {
        let all = punct_switches();
        assert_eq!(all.len(), 19);
        let mut keys: Vec<_> = all.iter().map(|(k, _)| k.clone()).collect();
        keys.dedup();
        assert_eq!(keys.len(), 19);
        assert_eq!(punct_switch_of(keys::char_key(',', keys::SHIFT), true).as_deref(), Some("win.punct.shift.comma"));
        assert_eq!(punct_switch_of(keys::char_key('[', 0), false).as_deref(), Some("win.punct.plain.lbracket"));
        assert_eq!(punct_switch_of(keys::char_key('\'', 0), true), None, "Eten's ' is Zhuyin");
        assert_eq!(punct_switch_of(keys::char_key(',', 0), true), None);
    }

    #[test]
    fn code_input_and_symbol_table() {
        let mut s = Special::default();
        s.open_code();
        for c in "u4f60".chars() {
            s.key(keys::char_key(c, 0));
        }
        assert!(matches!(s.key(keys::special(keys::SPACE, 0)), SpecialOut::Done(t) if t == "你"));
        assert!(!s.active());
        s.open_code();
        assert!(matches!(s.key(keys::char_key('s', 0)), SpecialOut::Pass));
        assert!(matches!(s.open_symbols(), SpecialOut::Show { ref candidates, .. } if candidates[0] == "，"));
        s.key(keys::special(keys::SPACE, 0));
        assert!(matches!(s.key(keys::char_key('1', 0)), SpecialOut::Done(t) if t == "「"));
    }
}
