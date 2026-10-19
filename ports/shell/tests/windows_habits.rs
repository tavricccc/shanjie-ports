//! The Windows habits (src/winops.rs, src/shell/win.rs; ports/docs/windows-habits.md): fake clients driving the shell
//! through the real core with the real data folder, as tests/shell.rs. The mixed input's cases are the old port's
//! (shanjie-multiplatform `platform/smart_input_windows.rs`, `windows_smart_tests`), on upstream's lexicon and English list.
use shanjie_shell::keys::{self, KeyInput};
use shanjie_shell::menu_tree;
use shanjie_shell::prefs::{key, Defaults, Prefs};
use shanjie_shell::shell::ProfileMode;
use shanjie_shell::winops::{key as wk, FLAGS};
use shanjie_shell::{ClientInfo, Effect, Effects, Modifier, PlatformTables, SessionId, Setup, Shell};
use std::path::PathBuf;

fn data() -> PathBuf {
    let dir = std::env::var_os("SHANJIE_TEST_DATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../build/ports/data"));
    assert!(dir.join("bigram.sjlm").is_file(), "{} has no data folder: run ports/data/fetch.sh && ports/data/assemble.sh build/ports/data", dir.display());
    dir
}

fn shell_with(prefs: Prefs, defaults: &Defaults) -> Shell {
    let setup = Setup {
        resources: data(),
        learning_directory: None,
        tables: PlatformTables { chat_apps: vec!["chat.app".into()], learning_denylist: vec![] },
        shows_abbreviation: false,
        punctuation: None,
        names: Default::default(),
    };
    let s = Shell::new(setup, prefs, defaults);
    assert!(s.has_engine());
    s
}

/// A shell with these habits on (and upstream's English mixing as `english`).
fn with(flags: &[&str], english: bool) -> World {
    let mut prefs = Prefs::memory();
    for f in flags {
        prefs.set_bool(f, true);
    }
    prefs.set_bool(key::ENGLISH, english);
    let mut w = World { shell: shell_with(prefs, &Defaults::upstream()), text: String::new(), marked: String::new(), hint: None, items: vec![] };
    w.shell.open_session(1, ClientInfo { app_id: Some("editor".into()), ..Default::default() });
    let mut out = Effects::new();
    w.shell.activate(1, &mut out);
    w.run(out);
    w
}

struct World {
    shell: Shell,
    text: String,
    marked: String,
    hint: Option<String>,
    items: Vec<String>,
}

impl World {
    fn run(&mut self, out: Effects) {
        for (_, e) in out {
            match e {
                Effect::Insert(t) => {
                    self.marked.clear();
                    self.text.push_str(&t);
                }
                Effect::Marked { text, .. } => self.marked = text,
                Effect::ClearMarked => self.marked.clear(),
                Effect::ShowHint { text, .. } => {
                    self.hint = Some(text);
                    self.items.clear();
                }
                Effect::ShowCandidates { candidates, .. } => {
                    self.hint = None;
                    self.items = candidates;
                }
                Effect::HideCandidates => {
                    self.hint = None;
                    self.items.clear();
                }
                _ => {}
            }
        }
    }

    fn key(&mut self, k: KeyInput) -> bool {
        let mut out = Effects::new();
        let h = self.shell.handle(1, k, &mut out);
        self.run(out);
        h
    }

    /// Whitespace-separated script: a single character is that keycap (upper case: with Shift), `_` Space, and the
    /// names below; `C-x` Control, `CA-x` Control+Alt, `S-_` Shift+Space, `C-_` Control+Space.
    fn keys(&mut self, script: &str) -> Vec<bool> {
        script.split_whitespace().map(|t| self.key(parse(t))).collect()
    }

    /// Characters typed one by one (`' '` is Space, upper case with Shift).
    fn typed(&mut self, s: &str) {
        for c in s.chars() {
            let k = if c == ' ' {
                KeyInput::Mapped(keys::special(keys::SPACE, 0))
            } else if c.is_ascii_uppercase() {
                KeyInput::Mapped(keys::char_key(c.to_ascii_lowercase(), keys::SHIFT))
            } else {
                KeyInput::Mapped(keys::char_key(c, 0))
            };
            self.key(k);
        }
    }

    fn modifier(&mut self, m: Modifier, down: bool) -> bool {
        let mut out = Effects::new();
        let r = self.shell.modifier(1, m, down, &mut out);
        self.run(out);
        r
    }

    fn menu(&mut self, key: &str, checked: Option<bool>) {
        let mut out = Effects::new();
        menu_tree::activate(&mut self.shell, 1, key, checked, &mut out);
        self.run(out);
    }
}

fn parse(t: &str) -> KeyInput {
    let special = |kind, m| KeyInput::Mapped(keys::special(kind, m));
    let (m, rest) = match t.split_once('-') {
        Some(("C", r)) if !r.is_empty() => (keys::CONTROL, r),
        Some(("CA", r)) if !r.is_empty() => (keys::CONTROL | keys::OPTION, r),
        Some(("S", r)) if !r.is_empty() => (keys::SHIFT, r),
        Some(("CS", r)) if !r.is_empty() => (keys::CONTROL | keys::SHIFT, r),
        Some(("Caps", r)) if !r.is_empty() => (keys::CAPSLOCK, r),
        _ => (0, t),
    };
    match rest {
        "_" => special(keys::SPACE, m),
        "Enter" => special(keys::ENTER, m),
        "BS" => special(keys::BACKSPACE, m),
        "Esc" => special(keys::ESC, m),
        "Down" => special(keys::DOWN, m),
        "Tab" => special(keys::TAB, m),
        "Left" => special(keys::LEFT, m),
        s => {
            let c = s.chars().next().unwrap();
            if c.is_ascii_uppercase() {
                KeyInput::Mapped(keys::char_key(c.to_ascii_lowercase(), m | keys::SHIFT))
            } else {
                KeyInput::Mapped(keys::char_key(c, m))
            }
        }
    }
}

// MARK: defaults and settings

#[test]
fn every_habit_starts_at_upstream_behavior() {
    let w = with(&[], true);
    for f in FLAGS {
        assert_eq!(w.shell.win_flag(f.key), f.upstream, "{}", f.key);
    }
    assert!(w.shell.chinese());
    assert!(shanjie_shell::winops::punct_switches().iter().all(|(k, _)| w.shell.punct_on(k)));
}

#[test]
fn overlay_sets_a_habit_until_it_is_chosen() {
    let dir = std::env::temp_dir().join(format!("shanjie-win-defaults-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join("defaults.ini");
    std::fs::write(&p, "win.shiftToggle=true\nwin.shiftSide=right\nwin.punct.shift.comma=false\nwin.unknown=true\nwin.mixedInput=maybe\n").unwrap();
    let (d, problems) = Defaults::load(&p);
    assert_eq!(problems.len(), 2, "an unknown key and a bad value are reported");
    let s = shell_with(Prefs::memory(), &d);
    assert!(s.win_flag(wk::SHIFT_TOGGLE));
    assert_eq!(s.shift_side(), shanjie_shell::winops::ShiftSide::Right);
    assert!(!s.punct_on("win.punct.shift.comma"));
    let mut prefs = Prefs::memory();
    prefs.set_bool(wk::SHIFT_TOGGLE, false);
    assert!(!shell_with(prefs, &d).win_flag(wk::SHIFT_TOGGLE), "a stored choice wins over the overlay");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn menu_has_the_two_submenus_and_stores_choices() {
    let mut w = with(&[], true);
    let tree = menu_tree::build(&w.shell, 1);
    let win = tree.iter().find(|n| n.key == "win").expect("Windows 操作");
    assert_eq!(win.kind, menu_tree::Kind::Submenu);
    assert!(win.children.iter().all(|n| n.children.is_empty()), "one level of submenus");
    for f in FLAGS {
        assert!(win.children.iter().any(|n| n.key == f.key), "{} in the menu", f.key);
    }
    let punct = tree.iter().find(|n| n.key == "win.punct").expect("Windows 操作：標點");
    assert_eq!(punct.children.iter().filter(|n| n.kind == menu_tree::Kind::Toggle).count(), 19);
    w.menu(wk::SHIFT_TOGGLE, None);
    assert!(w.shell.win_flag(wk::SHIFT_TOGGLE));
    w.menu(wk::SHIFT_TOGGLE, Some(true));
    assert!(w.shell.win_flag(wk::SHIFT_TOGGLE), "the same state again does nothing");
    w.menu("win.side.left", Some(true));
    assert_eq!(w.shell.shift_side(), shanjie_shell::winops::ShiftSide::Left);
    w.menu("win.punct.shift.comma", Some(false));
    assert!(!w.shell.punct_on("win.punct.shift.comma"));
    w.menu("win.punct.all.on", None);
    assert!(w.shell.punct_on("win.punct.shift.comma"));
}

#[test]
fn template_is_the_old_windows_profile_and_reset_goes_back() {
    let mut w = with(&[], true);
    w.menu("win.template.windows", None);
    for f in FLAGS.iter().filter(|f| f.key != wk::FULL_WIDTH) {
        assert_eq!(w.shell.win_flag(f.key), f.windows, "{}", f.key);
    }
    assert!(!w.shell.punct_on("win.punct.plain.lbracket"));
    let s = w.shell.settings();
    assert!(!s.prediction && !s.english);
    assert_eq!(s.profile_mode, ProfileMode::Chat);
    w.menu("win.template.upstream", None);
    for f in FLAGS {
        assert_eq!(w.shell.win_flag(f.key), f.upstream, "{}", f.key);
    }
    assert!(w.shell.punct_on("win.punct.plain.lbracket"));
}

// MARK: 中／英, 全形

#[test]
fn lone_shift_toggles_and_commits_first() {
    let mut w = with(&[wk::SHIFT_TOGGLE], true);
    w.keys("s u 3");
    assert_eq!(w.marked, "你");
    w.modifier(Modifier::Shift { right: false }, true);
    assert!(w.modifier(Modifier::Shift { right: false }, false));
    assert!(!w.shell.chinese());
    assert_eq!(w.text, "你", "what was typed is committed");
    assert_eq!(w.keys("s u 3"), vec![false; 3], "English: every key goes to the app");
    w.modifier(Modifier::Shift { right: true }, true);
    w.modifier(Modifier::Shift { right: true }, false);
    assert!(w.shell.chinese());
    // A held Shift repeats its press; still a tap. Both Shifts at once are not.
    w.modifier(Modifier::Shift { right: false }, true);
    w.modifier(Modifier::Shift { right: false }, true);
    assert!(w.modifier(Modifier::Shift { right: false }, false));
    assert!(!w.shell.chinese());
    w.modifier(Modifier::Shift { right: false }, true);
    w.modifier(Modifier::Shift { right: false }, false);
    assert!(w.shell.chinese());
    w.modifier(Modifier::Shift { right: false }, true);
    w.modifier(Modifier::Shift { right: true }, true);
    assert!(!w.modifier(Modifier::Shift { right: true }, false));
    assert!(!w.modifier(Modifier::Shift { right: false }, false));
    assert!(w.shell.chinese());
    // Shift used with a key is not a tap.
    w.modifier(Modifier::Shift { right: false }, true);
    w.keys("A");
    assert!(!w.modifier(Modifier::Shift { right: false }, false));
    assert!(w.shell.chinese());
    // Shift with Control is not a tap.
    w.modifier(Modifier::Shift { right: false }, true);
    w.modifier(Modifier::Other, true);
    assert!(!w.modifier(Modifier::Shift { right: false }, false));
    // Only the chosen side.
    w.menu("win.side.right", Some(true));
    w.modifier(Modifier::Shift { right: false }, true);
    assert!(!w.modifier(Modifier::Shift { right: false }, false));
    assert!(w.shell.chinese());
}

#[test]
fn lone_shift_does_nothing_while_off() {
    let mut w = with(&[], true);
    w.modifier(Modifier::Shift { right: false }, true);
    assert!(!w.modifier(Modifier::Shift { right: false }, false));
    assert!(w.shell.chinese());
}

#[test]
fn control_space_toggles_only_when_chosen() {
    let mut w = with(&[], true);
    assert!(!w.keys("C-_")[0], "upstream passes Control+Space to the app");
    let mut w = with(&[wk::CTRL_SPACE], true);
    assert!(w.keys("C-_")[0]);
    assert!(!w.shell.chinese());
}

#[test]
fn full_width_from_shift_space() {
    let mut w = with(&[wk::SHIFT_SPACE, wk::CTRL_SPACE], true);
    assert!(w.keys("S-_")[0]);
    assert!(w.shell.win_flag(wk::FULL_WIDTH));
    w.keys("_");
    assert_eq!(w.text, "\u{3000}", "Space with nothing typed is the ideographic space");
    w.keys("C-_ a A 1 S-1 _");
    assert_eq!(w.text, "\u{3000}ａＡ１！\u{3000}", "English keys are typed full width");
    w.keys("C-_ S-_ _");
    assert_eq!(w.text, "\u{3000}ａＡ１！\u{3000}", "half width again: Space goes to the app");
}

#[test]
fn caps_lock_types_zhuyin_when_it_is_not_english() {
    let mut w = with(&[], true);
    assert!(!w.keys("Caps-s")[0], "upstream: Caps Lock passes the key");
    let mut prefs = Prefs::memory();
    prefs.set_bool(wk::CAPS_ENGLISH, false);
    let mut w = World { shell: shell_with(prefs, &Defaults::upstream()), text: String::new(), marked: String::new(), hint: None, items: vec![] };
    w.shell.open_session(1, ClientInfo::default());
    w.keys("Caps-s Caps-u Caps-3");
    assert_eq!(w.marked, "你");
}

// MARK: punctuation

#[test]
fn punctuation_switch_off_sends_the_key_to_the_app() {
    let mut w = with(&[], true);
    w.menu("win.punct.shift.comma", Some(false));
    w.keys("s u 3");
    assert!(!w.keys("S-,")[0]);
    assert_eq!(w.text, "你", "the composition is committed before the key goes on");
    assert!(w.keys("S-.")[0], "other marks are unchanged");
    assert_eq!(w.marked, "。");
}

#[test]
fn control_punctuation_types_the_core_marks() {
    let mut w = with(&[wk::CTRL_PUNCT], true);
    w.keys("s u 3 C-, C-[ CS-1");
    assert_eq!(w.marked, "你，「！");
    let mut w = with(&[], true);
    assert!(!w.keys("C-,")[0], "off: Control+comma goes to the app");
}

// MARK: keys

#[test]
fn enter_on_a_candidate_commits_when_chosen() {
    let mut w = with(&[], true);
    w.keys("s u 3 c l 3 Down Enter");
    assert!(w.text.is_empty(), "upstream: the pick stays in the composition");
    let mut w = with(&[wk::ENTER_COMMITS], true);
    w.keys("s u 3 c l 3 Down Enter");
    assert_eq!(w.text, "你好嗎");
    assert!(w.marked.is_empty());
    w.keys("s u 3 c l 3 Enter");
    assert_eq!(w.text, "你好嗎你好", "Enter without a window commits as before");
}

#[test]
fn code_input_and_symbol_table() {
    let mut w = with(&[wk::CODE_INPUT, wk::SYMBOLS], true);
    w.keys("` u 4 f 6 0");
    assert_eq!(w.marked, "`U4F60");
    w.keys("_");
    assert_eq!(w.text, "你");
    w.keys("CA-,");
    assert_eq!(w.items.first().map(String::as_str), Some("，"));
    w.keys("2");
    assert_eq!(w.text, "你。");
    // A backquote not followed by U is the core's ·, then the key.
    w.keys("` s u 3 Enter");
    assert_eq!(w.text, "你。·你");
    let mut w = with(&[], true);
    w.keys("` Enter");
    assert_eq!(w.text, "·", "off: upstream's backquote");
}

// MARK: Windows 注音順序混打

fn mixed() -> World {
    with(&[wk::MIXED], false)
}

#[test]
fn mixed_shows_the_keys_and_the_reading_under_them() {
    let mut w = mixed();
    w.keys("g j");
    assert_eq!(w.marked, "gj");
    assert_eq!(w.hint.as_deref(), Some("ㄕㄨ"));
    w.keys("BS");
    assert_eq!(w.marked, "g");
    w.keys("J");
    assert_eq!(w.marked, "gJ");
    w.keys("Enter");
    assert_eq!(w.text, "gJ");
}

#[test]
fn mixed_tone_turns_the_keys_into_chinese() {
    let mut w = mixed();
    w.keys("s u 3");
    assert_eq!(w.marked, "你");
    w.keys("Enter");
    assert_eq!(w.text, "你");
}

#[test]
fn mixed_english_between_chinese() {
    let mut w = mixed();
    w.typed("su3hello");
    assert_eq!(w.marked, "你hello");
    w.typed("su3");
    assert_eq!(w.marked, "你hello你");
    w.keys("Enter");
    assert_eq!(w.text, "你hello你");
}

#[test]
fn mixed_english_before_chinese() {
    let mut w = mixed();
    w.typed("callsu3");
    assert_eq!(w.marked, "call你");
    w.keys("Enter");
    assert_eq!(w.text, "call你");
}

#[test]
fn mixed_english_words_and_space() {
    let mut w = mixed();
    w.typed("hello world");
    assert_eq!(w.marked, "hello world");
    w.keys("Enter");
    assert_eq!(w.text, "hello world");
}

#[test]
fn mixed_backspace_and_escape() {
    let mut w = mixed();
    w.typed("hello");
    w.keys("BS");
    assert_eq!(w.marked, "hell");
    w.keys("Esc");
    assert!(w.marked.is_empty());
    w.keys("Enter");
    assert!(w.text.is_empty());
}

#[test]
fn mixed_capitals_are_kept() {
    let mut w = mixed();
    w.typed("su3");
    w.typed("GitHub ");
    w.typed("cl3");
    assert_eq!(w.marked, "你GitHub 好");
    w.keys("Enter");
    assert_eq!(w.text, "你GitHub 好");
}

#[test]
fn mixed_zhuyin_order_matters_only_when_it_starts_with_a_letter() {
    let mut w = mixed();
    w.keys("d 0 4");
    assert_eq!(w.marked, "看");
    w.keys("Esc Esc 0 d 4");
    assert_eq!(w.marked, "看", "a digit first: any order");
}

#[test]
fn mixed_digits_stay_digits() {
    let mut w = mixed();
    w.keys("1 3");
    assert_eq!(w.marked, "13");
    w.keys("Enter");
    assert_eq!(w.text, "13");
    w.keys("1 8 4");
    assert!(w.marked.chars().all(shanjie_shell::shell::is_han) && !w.marked.is_empty(), "a whole syllable still converts: {}", w.marked);
}

#[test]
fn mixed_down_shows_the_zhuyin_candidates() {
    let mut w = mixed();
    w.keys("s u");
    assert!(w.keys("Down")[0]);
    assert_eq!(w.marked, "ㄋㄧ");
    assert!(!w.items.is_empty());
}

#[test]
fn mixed_tolerance_drops_one_repeated_key() {
    let mut w = with(&[wk::MIXED, wk::MIXED_TOLERANCE], false);
    w.typed("ccl3");
    assert_eq!(w.marked, "好");
    let mut w = mixed();
    w.typed("ccl3");
    assert_ne!(w.marked, "好", "off: the extra key stays");
}

#[test]
fn mixed_is_off_for_eten_and_leaves_upstream_alone() {
    let mut prefs = Prefs::memory();
    prefs.set_bool(wk::MIXED, true);
    prefs.set_string(key::LAYOUT, "eten");
    let s = shell_with(prefs, &Defaults::upstream());
    assert!(!s.mixed_available());
    let mut w = with(&[], true);
    w.keys("g j");
    assert_eq!(w.marked, "ㄕㄨ", "off: upstream's Zhuyin");
}

#[test]
fn mixed_with_upstream_english_leaves_shift_to_its_run() {
    let mut w = with(&[wk::MIXED], true);
    w.keys("R u s t");
    assert_eq!(w.marked, "Rust");
    w.keys("_ s u 3 Enter");
    assert_eq!(w.text, "Rust你");
}

#[test]
fn mixed_raw_keys_are_committed_on_focus_loss() {
    let mut w = mixed();
    w.typed("su3hel");
    let mut out = Effects::new();
    w.shell.deactivate(1 as SessionId, &mut out);
    w.run(out);
    assert_eq!(w.text, "你hel");
}


#[test]
fn overlay_merge_knows_every_habit() {
    let merge = std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../overlay/merge.sh")).unwrap();
    let known: Vec<&str> = merge.lines().filter_map(|l| l.strip_prefix("known=\"")).flat_map(|l| l.trim_end_matches('"').split(' ')).collect();
    let mut habits: Vec<String> = FLAGS.iter().map(|f| f.key.to_string()).collect();
    habits.push(wk::SHIFT_SIDE.to_string());
    habits.extend(shanjie_shell::winops::punct_switches().into_iter().map(|(k, _)| k));
    for k in &habits {
        assert!(known.contains(&k.as_str()), "ports/overlay/merge.sh does not accept {k}");
    }
    assert_eq!(known.iter().filter(|k| k.starts_with("win.")).count(), habits.len(), "merge.sh accepts a win.* key the shell does not know");
}
