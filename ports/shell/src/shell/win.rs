//! The Windows habits inside the shell (the table and the pure parts are in winops.rs; ports/docs/windows-habits.md).
//! Every habit acts before upstream's path in `Session.handle` and reaches the core only as keys it already takes.
//!
//! Windows 注音順序混打 is the old port's mixed input (shanjie-multiplatform `platform/smart_input_windows.rs`), its
//! rules unchanged, moved from the core into the shell: the keys of the syllable being typed stay in the shell (`mix_raw`)
//! and are shown as typed inside the core's preedit, with the reading as a hint under them. A tone (or Space) sends them
//! to the core as Zhuyin; keys that cannot be Zhuyin in the typed order are English, which goes into the composition as
//! upstream's English token (the core's English run, typed for it) or, when it holds more than letters, is committed
//! with the composition. The English word list is upstream's english-words.txt and the syllables are those of
//! upstream's lexicon, read from the data folder.
use super::*;
use crate::winops::{key as wk, ShiftSide, SpecialOut, FLAGS};
use shanjie_core::engine::Layout;

/// A modifier key pressed or released on its own, which the platform reports to `Shell::modifier` (單按 Shift).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Modifier {
    Shift { right: bool },
    /// Control, Alt, the Windows or Super key: a Shift held with one of them is not a tap.
    Other,
}

/// A lone Shift press, from its press to its release.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ShiftTap {
    Idle,
    Armed { right: bool },
    Spoiled,
}

impl ShiftTap {
    pub(super) fn spoil(&mut self) {
        if *self != ShiftTap::Idle {
            *self = ShiftTap::Spoiled;
        }
    }
}

const TONE_MARKS: [&str; 5] = ["", "ˊ", "ˇ", "ˋ", "˙"];

/// `s` with `ins` at UTF-16 offset `at` (clamped to the end).
fn splice(s: &str, at: u32, ins: &str) -> String {
    let mut units = 0u32;
    let byte = s.char_indices().find(|(_, c)| {
        let here = units >= at;
        units += c.len_utf16() as u32;
        here
    });
    let byte = byte.map_or(s.len(), |(i, _)| i);
    format!("{}{ins}{}", &s[..byte], &s[byte..])
}

fn utf16_len(s: &str) -> u32 {
    s.encode_utf16().count() as u32
}

/// The commit of a composition that showed `raw` at the core's cursor: the raw keys where they were shown. The core
/// commits what it showed, so they go in at the same offset; anything else (it never differs) gets them at the end.
pub(super) fn with_raw(commit: &str, core: &(String, u32), raw: &str) -> String {
    if commit == core.0 {
        splice(commit, core.1, raw)
    } else {
        format!("{commit}{raw}")
    }
}

impl Shell {
    // MARK: settings

    pub(super) fn load_win(&mut self, d: &Defaults) {
        let stored = |s: &Shell, k: &str| s.prefs.bool(k).or_else(|| d.win.get(k).and_then(|v| crate::prefs::parse_bool(v)));
        for f in FLAGS {
            let on = stored(self, f.key).unwrap_or(f.upstream);
            self.win_flags.insert(f.key, on);
        }
        for (k, _) in winops::punct_switches() {
            if !stored(self, &k).unwrap_or(true) {
                self.punct_off.insert(k);
            }
        }
        self.shift_side = self
            .prefs
            .string(wk::SHIFT_SIDE)
            .or_else(|| d.win.get(wk::SHIFT_SIDE).cloned())
            .and_then(|s| ShiftSide::from_raw(&s))
            .unwrap_or(ShiftSide::Both);
    }

    pub fn win_flag(&self, k: &str) -> bool {
        self.win_flags.get(k).copied().unwrap_or(false)
    }

    pub fn punct_on(&self, k: &str) -> bool {
        !self.punct_off.contains(k)
    }

    pub fn shift_side(&self) -> ShiftSide {
        self.shift_side
    }

    /// 中文 (true) or the keys going to the app as they are.
    pub fn chinese(&self) -> bool {
        self.chinese
    }

    /// Windows 注音順序混打 can run: the standard layout with the English word list, as upstream's English mixing.
    pub fn mixed_available(&self) -> bool {
        self.english_available && self.mode == InputMode::Standard
    }

    fn mixed_active(&self) -> bool {
        self.chinese && self.win_flag(wk::MIXED) && self.mixed_available()
    }

    /// The core's English mixing: on for upstream's switch, and for the Windows mixed input, which puts its English in
    /// as upstream's English tokens. Showing the letters as typed is upstream's alone: off while only the Windows
    /// mixed input turned the core's switch on, so the two never show the same keys twice.
    pub(super) fn core_english_inline(&self) -> bool {
        self.english_inline_on && self.english_on
    }

    pub(super) fn core_english(&self) -> Option<CoreResult> {
        let e = self.engine.as_ref()?;
        let _ = e.set_english_inline(self.core_english_inline());
        Some(e.set_english(self.english_on || self.win_flag(wk::MIXED)))
    }

    /// A switch of the Windows habits chosen in the menu.
    pub fn set_win_flag(&mut self, k: &str, on: bool, out: &mut Effects) {
        let Some(f) = FLAGS.iter().find(|f| f.key == k) else { return };
        if self.win_flag(f.key) == on {
            return;
        }
        if f.key == wk::MIXED && self.composing {
            match self.owner {
                Some(o) => self.finish(o, 0, out),
                None => self.discard_orphan(out),
            }
        }
        self.win_flags.insert(f.key, on);
        self.prefs.set_bool(f.key, on);
        if f.key == wk::MIXED {
            match self.owner {
                Some(o) => self.session_apply_setting(o, |s| s.core_english(), out, false),
                None => {
                    let r = self.core_english();
                    self.changed(out);
                    self.drop_on_failure(r, out)
                }
            }
            return;
        }
        self.changed(out);
    }

    pub fn set_shift_side(&mut self, side: ShiftSide, out: &mut Effects) {
        self.shift_side = side;
        self.prefs.set_string(wk::SHIFT_SIDE, side.raw());
        self.changed(out);
    }

    /// One punctuation switch (`win.punct.*`); on means the core makes the mark, off that the key goes to the app.
    pub fn set_punct(&mut self, k: &str, on: bool, out: &mut Effects) {
        if !winops::punct_switches().iter().any(|(p, _)| p == k) {
            return;
        }
        if on {
            self.punct_off.remove(k);
        } else {
            self.punct_off.insert(k.to_string());
        }
        self.prefs.set_bool(k, on);
        self.changed(out);
    }

    pub fn set_all_punct(&mut self, on: bool, out: &mut Effects) {
        for (k, _) in winops::punct_switches() {
            self.prefs.set_bool(&k, on);
            if on {
                self.punct_off.remove(&k);
            } else {
                self.punct_off.insert(k);
            }
        }
        self.changed(out);
    }

    /// 套用舊版 Windows 操作: the old port's Windows template (`profiles/windows.ini`), for the habits and for the three
    /// upstream settings that template changed (即時預測 off, 一律聊天, upstream's English mixing off).
    pub fn apply_windows_template(&mut self, out: &mut Effects) {
        for f in FLAGS {
            if f.key != wk::FULL_WIDTH {
                self.set_win_flag(f.key, f.windows, out);
            }
        }
        self.set_shift_side(ShiftSide::Both, out);
        self.set_all_punct(false, out);
        self.apply_prediction(false, out);
        self.apply_english(false, out);
        self.set_profile_mode(ProfileMode::Chat, out);
    }

    /// Windows 操作全部還原: every habit back to upstream's behavior. The upstream settings stay as they are.
    pub fn reset_windows_habits(&mut self, out: &mut Effects) {
        for f in FLAGS {
            self.set_win_flag(f.key, f.upstream, out);
        }
        self.set_shift_side(ShiftSide::Both, out);
        self.set_all_punct(true, out);
        if !self.chinese {
            self.chinese = true;
            self.changed(out);
        }
    }

    /// 中／英: what is being typed is committed first, as Microsoft Bopomofo does.
    pub fn toggle_chinese(&mut self, id: SessionId, out: &mut Effects) {
        if self.composing {
            match self.owner {
                Some(o) => self.finish(o, 0, out),
                None => self.discard_orphan(out),
            }
            self.hide_candidates(out);
        }
        self.chinese = !self.chinese;
        self.spacing_prev.remove(&id);
        self.changed(out);
    }

    /// A modifier key pressed or released on its own. Returns whether the shell acted on it; the platform passes the
    /// event on either way (a released Shift does nothing in an app).
    pub fn modifier(&mut self, id: SessionId, m: Modifier, down: bool, out: &mut Effects) -> bool {
        match (m, down) {
            (Modifier::Shift { right }, true) => {
                // The same Shift again is the key repeating while held.
                self.shift_tap = match self.shift_tap {
                    ShiftTap::Idle => ShiftTap::Armed { right },
                    ShiftTap::Armed { right: r } if r == right => ShiftTap::Armed { right },
                    _ => ShiftTap::Spoiled,
                };
                false
            }
            (Modifier::Shift { right }, false) => {
                let tap = std::mem::replace(&mut self.shift_tap, ShiftTap::Idle);
                if tap == (ShiftTap::Armed { right }) && self.win_flag(wk::SHIFT_TOGGLE) && self.shift_side.allows(right) && self.engine.is_some() {
                    self.toggle_chinese(id, out);
                    return true;
                }
                false
            }
            (Modifier::Other, true) => {
                self.shift_tap.spoil();
                false
            }
            (Modifier::Other, false) => false,
        }
    }

    pub(super) fn win_clear(&mut self) {
        self.mix_raw.clear();
        self.mix_latin = false;
        self.special.close();
        self.core_text = (String::new(), 0);
        self.core_run = false;
    }

    // MARK: keys

    /// Caps Lock 打英文 off: the core never sees Caps Lock (it would pass the key to the app).
    pub(super) fn win_input(&self, input: KeyInput) -> KeyInput {
        if self.win_flag(wk::CAPS_ENGLISH) {
            return input;
        }
        match input {
            KeyInput::Mapped(mut k) => {
                k.modifiers &= !keys::CAPSLOCK;
                KeyInput::Mapped(k)
            }
            KeyInput::Page { up, modifiers } => KeyInput::Page { up, modifiers: modifiers & !keys::CAPSLOCK },
            other => other,
        }
    }

    /// The habits' part of `handle`: `Some(handled)` when a habit took the key, `None` for upstream's path.
    pub(super) fn win_key(&mut self, id: SessionId, input: KeyInput, out: &mut Effects) -> Option<bool> {
        let k = match input {
            KeyInput::Mapped(k) => k,
            _ if !self.chinese => {
                if self.composing {
                    self.finish(id, 0, out);
                }
                return Some(false);
            }
            KeyInput::Page { .. } if self.special.active() => return Some(true),
            _ => return None,
        };
        let m = k.modifiers & !keys::CAPSLOCK;
        if k.kind == keys::SPACE && m == keys::CONTROL && self.win_flag(wk::CTRL_SPACE) {
            self.toggle_chinese(id, out);
            return Some(true);
        }
        if k.kind == keys::SPACE && m == keys::SHIFT && self.win_flag(wk::SHIFT_SPACE) {
            let on = !self.win_flag(wk::FULL_WIDTH);
            self.set_win_flag(wk::FULL_WIDTH, on, out);
            return Some(true);
        }
        if !self.chinese {
            return Some(self.english_key(id, k, out));
        }
        if self.special.active() {
            return Some(self.special_key(id, k, out));
        }
        if self.win_flag(wk::SYMBOLS) && k.kind == keys::CHAR && k.ch == ',' as u32 && m == keys::CONTROL | keys::OPTION {
            if self.composing {
                self.finish(id, 0, out);
            }
            let r = self.special.open_symbols();
            self.special_out(id, r, out);
            return Some(true);
        }
        if self.win_flag(wk::CODE_INPUT) && k.kind == keys::CHAR && k.ch == '`' as u32 && m == 0 && !self.composing {
            let r = self.special.open_code();
            self.special_out(id, r, out);
            return Some(true);
        }
        if self.win_flag(wk::CTRL_PUNCT) {
            if let Some(t) = winops::ctrl_punct(keys::ShanjieKey { modifiers: m, ..k }) {
                self.mix_flush(id, out);
                return Some(self.send(id, t, out));
            }
        }
        let c = char::from_u32(k.ch).unwrap_or('\0');
        let zhuyin = self.core_layout().symbol_of(c).is_some() || self.tone_of(c).is_some();
        if let Some(switch) = winops::punct_switch_of(keys::ShanjieKey { modifiers: m, ..k }, zhuyin) {
            if !self.punct_on(&switch) {
                if self.composing {
                    self.finish(id, 0, out);
                }
                self.spacing_prev.remove(&id);
                return Some(false);
            }
        }
        // 全形 with nothing being typed: Space is the ideographic space, as in Microsoft Bopomofo's full-width mode.
        if self.win_flag(wk::FULL_WIDTH) && k.kind == keys::SPACE && m == 0 && !self.composing {
            self.spacing_prev.remove(&id);
            self.insert(id, " ", out);
            return Some(true);
        }
        if self.mixed_active() {
            if let Some(handled) = self.mix_key(id, k, out) {
                return Some(handled);
            }
        }
        if self.win_flag(wk::ENTER_COMMITS) && k.kind == keys::ENTER && m == 0 && self.cands_open {
            let handled = self.send(id, k, out);
            if self.composing && !self.cands_open {
                self.send(id, k, out);
            }
            return Some(handled);
        }
        None
    }

    /// A key while the keys go to the app as they are: 全形 types the full-width character itself.
    fn english_key(&mut self, id: SessionId, k: keys::ShanjieKey, out: &mut Effects) -> bool {
        if self.composing {
            self.finish(id, 0, out);
        }
        self.spacing_prev.remove(&id);
        if self.win_flag(wk::FULL_WIDTH) {
            if let Some(c) = winops::typed_char(k) {
                self.insert(id, &c.to_string(), out);
                return true;
            }
        }
        false
    }

    /// A key while the symbol table or the code input is open.
    fn special_key(&mut self, id: SessionId, k: keys::ShanjieKey, out: &mut Effects) -> bool {
        match self.special.key(k) {
            SpecialOut::Pass => {
                // The backquote was meant as itself: the core's mark (or the key itself, with its switch off), then the key.
                self.composing = false;
                out.push((id, Effect::ClearMarked));
                if self.punct_on(&winops::punct_key(false, "backquote")) {
                    self.send(id, keys::char_key('`', 0), out);
                } else {
                    self.insert(id, "`", out);
                }
                match self.win_key(id, KeyInput::Mapped(k), out) {
                    Some(handled) => handled,
                    None => self.send(id, k, out),
                }
            }
            r => {
                self.special_out(id, r, out);
                true
            }
        }
    }

    pub(super) fn special_out(&mut self, id: SessionId, r: SpecialOut, out: &mut Effects) {
        match r {
            SpecialOut::Show { marked, candidates, selected, first, total } => {
                out.push((id, Effect::Marked { cursor_utf16: utf16_len(&marked), text: marked, run_start: 0, run_len: 0 }));
                self.composing = true;
                if candidates.is_empty() {
                    self.hide_candidates(out);
                } else {
                    let vertical = u32::from(self.candidate_vertical);
                    self.shown_vertical = vertical;
                    self.hint_shown = false;
                    self.cands_open = true;
                    self.panel_session = Some(id);
                    let notes = vec![None; candidates.len()];
                    out.push((id, Effect::ShowCandidates { candidates, notes, selected: selected as i32, columns: 0, first, total, vertical, anchor_utf16: 0 }));
                }
            }
            SpecialOut::Done(text) => {
                if !text.is_empty() {
                    self.insert(id, &text, out);
                }
                if self.composing {
                    out.push((id, Effect::ClearMarked));
                }
                self.composing = false;
                self.hide_candidates(out);
            }
            SpecialOut::Pass => {}
        }
    }

    // MARK: Windows 注音順序混打

    fn core_layout(&self) -> Layout {
        match self.mode {
            InputMode::Standard => Layout::Standard,
            InputMode::Eten => Layout::Eten,
        }
    }

    /// Tone 1..=4 of a tone key in the current layout.
    fn tone_of(&self, c: char) -> Option<usize> {
        ['ˊ', 'ˇ', 'ˋ', '˙'].iter().position(|m| self.core_layout().key_of_tone(*m) == Some(c)).map(|i| i + 1)
    }

    /// The tone a key gives (Space is tone 1, index 0).
    fn mix_tone(&self, k: keys::ShanjieKey) -> Option<usize> {
        match k.kind {
            keys::SPACE if k.modifiers == 0 => Some(0),
            keys::CHAR if k.modifiers == 0 => self.tone_of(char::from_u32(k.ch)?),
            _ => None,
        }
    }

    /// Each column (initial, medial, final) at most once. A run starting with a letter must also follow initial, medial,
    /// final, so an English word does not turn into Zhuyin; one starting with a digit or symbol (0d4 = ㄎㄢˋ) may come in
    /// any order (the old port's `mixed_reading`).
    fn reading_order(&self, raw: &str, ordered: bool) -> Option<String> {
        let mut last = None;
        let mut columns: [Option<char>; 3] = [None; 3];
        for c in raw.chars() {
            let (column, symbol) = self.core_layout().symbol_of(c.to_ascii_lowercase())?;
            if columns[column].is_some() || ordered && last.is_some_and(|previous| previous >= column) {
                return None;
            }
            last = Some(column);
            columns[column] = Some(symbol);
        }
        let reading: String = columns.iter().flatten().collect();
        (!reading.is_empty()).then_some(reading)
    }

    fn reading(&self, raw: &str) -> Option<String> {
        self.reading_order(raw, raw.chars().next().is_some_and(|c| c.is_ascii_alphabetic()))
    }

    /// Whether the lexicon has a word for the syllable `reading` with `tone`: the one-syllable readings of upstream's
    /// mcbpmf-data.txt, read once.
    fn syllable(&mut self, reading: &str, tone: usize) -> bool {
        if self.syllables.is_none() {
            let text = std::fs::read_to_string(self.resources.join("mcbpmf-data.txt")).unwrap_or_default();
            let set = text
                .lines()
                .filter(|l| !l.starts_with(['#', '_']))
                .filter_map(|l| l.split_whitespace().next())
                .filter(|r| !r.contains('-'))
                .map(str::to_string)
                .collect();
            self.syllables = Some(set);
        }
        self.syllables.as_ref().is_some_and(|s| s.contains(&format!("{reading}{}", TONE_MARKS[tone])))
    }

    /// Whether `w` (any case) is in upstream's English word list, read once.
    fn english_word(&mut self, w: &str) -> bool {
        if self.english_words.is_none() {
            let text = std::fs::read_to_string(self.resources.join("english-words.txt")).unwrap_or_default();
            let mut words: Vec<String> = text.lines().map(str::to_string).collect();
            words.sort();
            self.english_words = Some(words);
        }
        let w = w.to_ascii_lowercase();
        self.english_words.as_ref().is_some_and(|l| l.binary_search(&w).is_ok())
    }

    /// 容許多按一次同一鍵: one adjacent repeated key dropped gives exactly one syllable with this tone (the old port's
    /// `corrected_mixed_keys`): 2 to 4 lower-case keys that are not an English word.
    fn corrected(&mut self, raw: &str, tone: usize) -> Option<String> {
        if !self.win_flag(wk::MIXED_TOLERANCE) || !(2..=4).contains(&raw.len()) || !raw.is_ascii() || raw.chars().any(|c| c.is_ascii_uppercase()) || self.english_word(raw) {
            return None;
        }
        let mut choices = std::collections::BTreeSet::new();
        for i in 1..raw.len() {
            if raw.as_bytes()[i] != raw.as_bytes()[i - 1] {
                continue;
            }
            let keys = format!("{}{}", &raw[..i], &raw[i + 1..]);
            if let Some(reading) = self.reading_order(&keys, false) {
                if self.syllable(&reading, tone) {
                    choices.insert(keys);
                }
            }
        }
        if choices.len() == 1 {
            choices.into_iter().next()
        } else {
            None
        }
    }

    /// The core's unfinished syllable shows as Zhuyin before its cursor.
    fn core_pending(&self) -> bool {
        let (text, cursor) = &self.core_text;
        let before = splice(text, *cursor, "\u{0}");
        let before = &before[..before.find('\u{0}').unwrap_or(0)];
        before.chars().last().is_some_and(|c| matches!(c as u32, 0x3105..=0x312F | 0x31A0..=0x31BF | 0x02CA | 0x02C7 | 0x02CB | 0x02D9))
    }

    /// The raw keys shown inside the core's preedit at its cursor (English underlined as upstream's run), and the
    /// reading under them while they can still be Zhuyin.
    fn mix_show(&mut self, id: SessionId, out: &mut Effects) {
        let (base, cursor) = self.core_text.clone();
        if self.mix_raw.is_empty() {
            if base.is_empty() {
                if self.composing {
                    out.push((id, Effect::ClearMarked));
                }
                self.composing = false;
            } else {
                out.push((id, Effect::Marked { text: base, cursor_utf16: cursor, run_start: 0, run_len: 0 }));
                self.composing = true;
            }
            self.hide_candidates(out);
            return;
        }
        let n = utf16_len(&self.mix_raw);
        let (run_start, run_len) = if self.mix_latin { (cursor, n) } else { (0, 0) };
        out.push((id, Effect::Marked { text: splice(&base, cursor, &self.mix_raw), cursor_utf16: cursor + n, run_start, run_len }));
        self.composing = true;
        match self.reading(&self.mix_raw).filter(|_| !self.mix_latin) {
            Some(r) => self.show_hint_text(id, &r, cursor, out),
            None => self.hide_candidates(out),
        }
    }

    /// Keys to the core one after another, as one output: the commits joined, the last snapshot shown.
    fn replay(&mut self, id: SessionId, keys: Vec<keys::ShanjieKey>, out: &mut Effects) -> bool {
        let Some(first) = keys.first().copied() else { return true };
        let paused = self.apply_learning_gate(id);
        let shown = self.composing;
        self.composing = !self.core_text.0.is_empty();
        self.prime(id, first, paused);
        self.composing = shown;
        let mut commit = String::new();
        let mut last = None;
        for k in keys {
            match self.engine.as_ref().map(|e| e.key(k)) {
                Some(CoreResult::Ok(o)) => {
                    commit.push_str(&o.commit);
                    last = Some(o);
                }
                Some(CoreResult::Failed(c)) => return self.fail(id, c, out),
                None => return false,
            }
        }
        if let Some(mut o) = last {
            o.commit = commit;
            self.apply(id, o, out);
        }
        true
    }

    /// The raw keys as Zhuyin keys, then `tail`. Initial, medial, final in that order: the core fills the same columns
    /// in any order, but with English mixing on it takes a digit key that comes first for a number (0d4 would be 0ㄎ).
    fn replay_raw(&mut self, id: SessionId, raw: &str, tail: Option<keys::ShanjieKey>, out: &mut Effects) -> bool {
        let mut chars: Vec<char> = raw.chars().map(|c| c.to_ascii_lowercase()).collect();
        if self.reading_order(raw, false).is_some() {
            chars.sort_by_key(|c| self.core_layout().symbol_of(*c).map_or(3, |(column, _)| column));
        }
        let mut ks: Vec<_> = chars.into_iter().map(|c| keys::char_key(c, 0)).collect();
        ks.extend(tail);
        self.replay(id, ks, out)
    }

    /// English text into the composition at the cursor: letters as upstream's English token (its run, typed: a letter,
    /// Shift and the next one to open the run on it, Backspace, the rest, Space to end it); anything else is committed
    /// with the composition, where it was shown.
    fn mix_literal(&mut self, id: SessionId, text: &str, out: &mut Effects) {
        if text.is_empty() {
            return;
        }
        if !text.chars().all(|c| c.is_ascii_alphabetic()) {
            self.mix_raw = text.to_string();
            self.finish(id, 0, out);
            return;
        }
        let chars: Vec<char> = text.chars().collect();
        let key_of = |c: char| keys::char_key(c.to_ascii_lowercase(), if c.is_ascii_uppercase() { keys::SHIFT } else { 0 });
        let mut ks = Vec::new();
        if chars[0].is_ascii_uppercase() {
            ks.push(key_of(chars[0]));
        } else {
            ks.push(keys::char_key(chars[0], 0));
            ks.push(keys::char_key(chars.get(1).map_or('a', |c| c.to_ascii_lowercase()), keys::SHIFT));
            ks.push(keys::special(keys::BACKSPACE, 0));
        }
        ks.extend(chars[1..].iter().map(|c| key_of(*c)));
        ks.push(keys::special(keys::SPACE, 0));
        self.replay(id, ks, out);
    }

    /// Raw keys waiting when another habit takes the key: what they are now (English, or Zhuyin not finished) goes in.
    fn mix_flush(&mut self, id: SessionId, out: &mut Effects) {
        let raw = std::mem::take(&mut self.mix_raw);
        let latin = std::mem::replace(&mut self.mix_latin, false);
        if raw.is_empty() {
            return;
        }
        if latin {
            self.mix_literal(id, &raw, out);
        } else {
            self.replay_raw(id, &raw, None, out);
        }
    }

    /// The old port's `mixed_key`. `None`: the key goes on as upstream's.
    fn mix_key(&mut self, id: SessionId, k: keys::ShanjieKey, out: &mut Effects) -> Option<bool> {
        if self.mix_raw.is_empty() && (self.cands_open || self.core_run || self.core_pending()) {
            return None;
        }
        let c = char::from_u32(k.ch).unwrap_or('\0');
        let is_char = k.kind == keys::CHAR;
        let plain = k.modifiers == 0;
        let zhuyin_key = is_char && plain && self.core_layout().symbol_of(c).is_some();
        if self.mix_latin {
            if k.modifiers & !keys::SHIFT != 0 {
                self.mix_flush(id, out);
                return None;
            }
            if is_char && c.is_ascii_alphabetic() {
                self.mix_raw.push(if k.modifiers == keys::SHIFT { c.to_ascii_uppercase() } else { c });
                self.mix_show(id, out);
                return Some(true);
            }
            if zhuyin_key {
                // Zhuyin also uses digits and punctuation (5j/ = ㄓㄨㄥ): keep a possible syllable until its tone.
                let next = format!("{}{c}", self.mix_raw);
                if (1..next.len()).any(|split| self.reading(&next[split..]).is_some()) {
                    self.mix_raw = next;
                    self.mix_show(id, out);
                    return Some(true);
                }
            }
            match k.kind {
                keys::BACKSPACE => {
                    self.mix_raw.pop();
                    self.mix_latin = !self.mix_raw.is_empty();
                    self.mix_show(id, out);
                    return Some(true);
                }
                keys::ESC => {
                    self.mix_raw.clear();
                    self.mix_latin = false;
                    self.mix_show(id, out);
                    return Some(true);
                }
                keys::DOWN | keys::TAB => {
                    let raw = std::mem::take(&mut self.mix_raw);
                    self.mix_latin = false;
                    return Some(self.replay_raw(id, &raw, Some(keys::special(keys::DOWN, 0)), out));
                }
                _ => return Some(self.finish_latin(id, k, out)),
            }
        }
        // Shift+letter starts English (with upstream's English mixing on, it starts upstream's run instead).
        if !self.english_on && is_char && k.modifiers == keys::SHIFT && c.is_ascii_alphabetic() && !self.cands_open {
            self.mix_raw.push(c.to_ascii_uppercase());
            self.mix_latin = true;
            self.mix_show(id, out);
            return Some(true);
        }
        if !self.mix_raw.is_empty() && plain {
            match k.kind {
                keys::ENTER => {
                    self.finish(id, 0, out);
                    return Some(true);
                }
                keys::BACKSPACE => {
                    self.mix_raw.pop();
                    self.mix_show(id, out);
                    return Some(true);
                }
                keys::DOWN | keys::TAB => {
                    let raw = std::mem::take(&mut self.mix_raw);
                    return Some(self.replay_raw(id, &raw, Some(k), out));
                }
                _ => {}
            }
        }
        if zhuyin_key {
            self.mix_raw.push(c);
            self.mix_latin = self.reading(&self.mix_raw).is_none();
            self.mix_show(id, out);
            return Some(true);
        }
        if self.mix_raw.is_empty() {
            return None;
        }
        let raw = std::mem::take(&mut self.mix_raw);
        if let Some(tone) = self.mix_tone(k) {
            // An incomplete initial such as 1 (ㄅ) has no syllable for any tone: the digits stay digits.
            if is_char && raw.bytes().all(|b| b.is_ascii_digit()) {
                let complete = self.reading_order(&raw, false).is_some_and(|r| self.syllable(&r, tone));
                if !complete {
                    self.mix_raw = format!("{raw}{c}");
                    self.mix_latin = true;
                    self.mix_show(id, out);
                    return Some(true);
                }
            }
            return Some(self.replay_raw(id, &raw, Some(k), out));
        }
        // Any other key: the unfinished syllable to the core, then the key as upstream's.
        self.replay_raw(id, &raw, None, out);
        Some(self.send(id, k, out))
    }

    /// The old port's `finish_latin`: a key that ends English. A tone may still make a syllable of the last keys.
    fn finish_latin(&mut self, id: SessionId, k: keys::ShanjieKey, out: &mut Effects) -> bool {
        let raw = std::mem::take(&mut self.mix_raw);
        self.mix_latin = false;
        let c = char::from_u32(k.ch).unwrap_or('\0');
        let plain_char = k.kind == keys::CHAR && k.modifiers == 0;
        // Once a run is digits, later tone-number keys stay digits.
        if !raw.is_empty() && raw.bytes().all(|b| b.is_ascii_digit()) && plain_char && c.is_ascii_digit() {
            self.mix_raw = format!("{raw}{c}");
            self.mix_latin = true;
            self.mix_show(id, out);
            return true;
        }
        let space = k.kind == keys::SPACE;
        if let Some(tone) = self.mix_tone(k).filter(|_| !space || !self.english_word(&raw)) {
            if let Some(fixed) = self.corrected(&raw, tone) {
                return self.replay_raw(id, &fixed, Some(k), out);
            }
            // A tone confirms a whole syllable typed in any order.
            if !space && self.reading_order(&raw, false).is_some_and(|r| self.syllable(&r, tone)) {
                return self.replay_raw(id, &raw, Some(k), out);
            }
            // Keep every English prefix; only a valid last syllable becomes Chinese.
            for split in raw.len().saturating_sub(3).max(1)..raw.len() {
                let (word, tail) = raw.split_at(split);
                if space && !self.english_word(word) && tail.chars().all(|c| c.is_ascii_alphabetic()) {
                    continue;
                }
                if self.reading(tail).is_some_and(|r| self.syllable(&r, tone)) {
                    self.mix_literal(id, word, out);
                    return self.replay_raw(id, tail, Some(k), out);
                }
            }
        }
        if plain_char && c.is_ascii_digit() {
            self.mix_raw = format!("{raw}{c}");
            self.mix_latin = true;
            self.mix_show(id, out);
            return true;
        }
        if plain_char && c.is_ascii_punctuation() {
            // English followed by a symbol: both as typed.
            self.mix_literal(id, &format!("{raw}{c}"), out);
            return true;
        }
        self.mix_literal(id, &raw, out);
        self.send(id, k, out)
    }
}
