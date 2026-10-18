//! Ports of macos/Tests/ShanjieKitTests/ShellTests.swift and friends: fake clients driving the shell through the real
//! core with the real data folder (`ports/data/assemble.sh build/ports/data`, or `SHANJIE_TEST_DATA`). No engine is faked.
use shanjie_shell::keys::{self, KeyInput};
use shanjie_shell::menu::{text, Action, InputMode, MenuEntry};
use shanjie_shell::prefs::{key, Defaults, Prefs};
use shanjie_shell::shell::{ProfileMode, RerankLevel};
use shanjie_shell::{ClientInfo, Effect, Effects, LeftText, PlatformTables, SessionId, Setup, Shell};
use std::path::PathBuf;

fn data() -> PathBuf {
    let dir = std::env::var_os("SHANJIE_TEST_DATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../build/ports/data"));
    assert!(
        dir.join("bigram.sjlm").is_file(),
        "{} has no data folder: run ports/data/fetch.sh && ports/data/assemble.sh build/ports/data",
        dir.display()
    );
    dir
}

fn tables() -> PlatformTables {
    PlatformTables { chat_apps: vec!["chat.app".into()], learning_denylist: vec!["password.app".into()] }
}

fn setup() -> Setup {
    Setup { resources: data(), learning_directory: None, tables: tables(), shows_abbreviation: false, punctuation: None, names: Default::default() }
}

fn shell_with(prefs: Prefs) -> Shell {
    let s = Shell::new(setup(), prefs, &Defaults::upstream());
    assert!(s.has_engine());
    s
}

fn shell() -> Shell {
    shell_with(Prefs::memory())
}

/// A text view stand-in (Support.swift `FakeClient` and `FakePanel`): insert replaces the marked text.
#[derive(Default)]
struct View {
    text: String,
    marked: String,
    inserts: Vec<String>,
    visible: bool,
    items: Vec<String>,
    selected: i32,
    hint: Option<String>,
    run: (u32, u32),
    confirm: usize,
}

struct World {
    shell: Shell,
    views: std::collections::HashMap<SessionId, View>,
}

impl World {
    fn new(shell: Shell) -> World {
        World { shell, views: Default::default() }
    }

    fn open(&mut self, id: SessionId, app: Option<&str>) {
        self.shell.open_session(id, ClientInfo { app_id: app.map(str::to_string), ..Default::default() });
        self.views.insert(id, View::default());
    }

    fn view(&self, id: SessionId) -> &View {
        &self.views[&id]
    }

    fn run(&mut self, out: Effects) {
        for (id, e) in out {
            let Some(v) = self.views.get_mut(&id) else { continue };
            match e {
                Effect::Insert(t) => {
                    v.marked.clear();
                    v.text.push_str(&t);
                    v.inserts.push(t);
                }
                Effect::Marked { text, run_start, run_len, .. } => {
                    v.marked = text;
                    v.run = (run_start, run_len);
                }
                Effect::ClearMarked => v.marked.clear(),
                Effect::ShowCandidates { candidates, selected, .. } => {
                    v.visible = true;
                    v.items = candidates;
                    v.selected = selected;
                    v.hint = None;
                }
                Effect::ShowHint { text, .. } => {
                    v.visible = true;
                    v.items = vec![];
                    v.hint = Some(text);
                }
                Effect::HideCandidates => {
                    v.visible = false;
                    v.items.clear();
                    v.hint = None;
                }
                Effect::ConfirmClear => v.confirm += 1,
                _ => {}
            }
        }
    }

    fn activate(&mut self, id: SessionId) {
        let mut out = Effects::new();
        self.shell.activate(id, &mut out);
        self.run(out);
    }

    fn press(&mut self, id: SessionId, input: KeyInput) -> bool {
        let mut out = Effects::new();
        let handled = self.shell.handle(id, input, &mut out);
        self.run(out);
        handled
    }

    fn type_keys(&mut self, id: SessionId, keys: &str) -> Vec<bool> {
        keys.chars().map(|c| self.press(id, ch(c, 0))).collect()
    }

    fn enter(&mut self, id: SessionId, modifiers: u32) -> bool {
        self.press(id, KeyInput::Mapped(keys::special(keys::ENTER, modifiers)))
    }

    fn perform(&mut self, id: SessionId, action: Action) {
        let mut out = Effects::new();
        self.shell.perform(id, action, &mut out);
        self.run(out);
    }
}

fn ch(c: char, modifiers: u32) -> KeyInput {
    if c == ' ' {
        KeyInput::Mapped(keys::special(keys::SPACE, modifiers))
    } else {
        KeyInput::Mapped(keys::char_key(c, modifiers))
    }
}

/// Selftest.swift `row284Standard` and its two sentences (Support.swift `Row284`).
const ROW284: &str = "tj6z;6vup 5j; xk7u ek7xm4gjo3cj6";
const ROW284_CHAT: &str = "廚房新裝了一個率水壺";
const ROW284_FORMAL: &str = "廚房新裝了一個律水壺";

#[test]
fn type_nihao_then_enter_commits() {
    let mut w = World::new(shell());
    w.open(1, Some("editor"));
    w.activate(1);
    assert_eq!(w.type_keys(1, "su3cl3"), vec![true; 6]);
    assert_eq!(w.view(1).marked, "你好");
    assert!(w.enter(1, 0));
    assert_eq!(w.view(1).inserts, vec!["你好".to_string()]);
    assert_eq!(w.view(1).marked, "");
    assert!(!w.view(1).visible);
}

#[test]
fn shift_enter_commits_then_passes_the_key_on() {
    let mut prefs = Prefs::memory();
    prefs.set_bool(key::ENGLISH, false);
    let mut w = World::new(shell_with(prefs));
    w.open(1, Some("editor"));
    w.activate(1);
    w.type_keys(1, "su3cl3");
    assert!(!w.enter(1, keys::SHIFT));
    assert_eq!(w.view(1).text, "你好");
    w.type_keys(1, "1");
    assert_eq!(w.view(1).marked, "ㄅ");
    assert!(!w.enter(1, keys::SHIFT));
    assert_eq!(w.view(1).text, "你好ㄅ");
    w.type_keys(1, "su3a");
    assert!(w.enter(1, 0));
    assert_eq!(w.view(1).text, "你好ㄅ你ㄇ");
}

#[test]
fn candidates_show_then_number_selects() {
    let mut w = World::new(shell());
    w.open(1, Some("editor"));
    w.activate(1);
    w.type_keys(1, "su3 ");
    assert!(w.view(1).visible);
    assert_eq!(w.view(1).selected, 0);
    let second = w.view(1).items[1].clone();
    assert!(w.type_keys(1, "2")[0]);
    assert_eq!(w.view(1).marked, second);
    assert!(!w.view(1).visible);
    assert_eq!(w.view(1).text, "");
}

#[test]
fn prediction_row_is_not_entered_until_tab() {
    let mut w = World::new(shell());
    w.open(1, Some("editor"));
    w.activate(1);
    w.type_keys(1, "s");
    assert!(w.view(1).visible);
    assert_eq!(w.view(1).selected, -1);
    assert!(w.press(1, KeyInput::Mapped(keys::special(keys::TAB, 0))));
    assert_eq!(w.view(1).selected, 0);
}

#[test]
fn caps_lock_key_passes_without_changing_anything() {
    let mut w = World::new(shell());
    w.open(1, Some("editor"));
    w.activate(1);
    w.type_keys(1, "su3");
    assert!(!w.enter(1, keys::CAPSLOCK));
    w.type_keys(1, "cl3");
    w.enter(1, 0);
    assert_eq!(w.view(1).text, "你好");
}

#[test]
fn key_outside_the_table_commits_then_passes() {
    let mut w = World::new(shell());
    w.open(1, Some("editor"));
    w.activate(1);
    w.type_keys(1, "su3");
    assert!(!w.press(1, KeyInput::Unmapped));
    assert_eq!(w.view(1).text, "你");
    assert_eq!(w.view(1).marked, "");
    let before = w.view(1).inserts.len();
    assert!(!w.press(1, KeyInput::Unmapped));
    assert_eq!(w.view(1).inserts.len(), before);
}

#[test]
fn page_keys_reach_the_core_only_in_the_vertical_window() {
    let mut prefs = Prefs::memory();
    prefs.set_bool(key::CANDIDATE_VERTICAL, true);
    let mut w = World::new(shell_with(prefs));
    w.open(1, Some("editor"));
    w.activate(1);
    w.type_keys(1, "g4 ");
    assert!(w.shell.vertical_open());
    let first = w.view(1).items.clone();
    assert!(w.press(1, KeyInput::Page { up: false, modifiers: 0 }));
    assert_ne!(w.view(1).items, first, "Page Down moved the window by nine");
    assert!(!w.press(1, KeyInput::Page { up: true, modifiers: keys::SHIFT }), "a modifier: outside the tables");
    assert_eq!(w.view(1).marked, "", "a key outside the tables commits first");
    assert_ne!(w.view(1).text, "");
}

#[test]
fn commit_composition() {
    let mut w = World::new(shell());
    w.open(1, Some("editor"));
    w.activate(1);
    w.type_keys(1, "su3cl3 ");
    assert!(w.view(1).visible);
    let mut out = Effects::new();
    w.shell.commit_composition(1, &mut out);
    w.run(out);
    assert_eq!(w.view(1).text, "你好");
    assert!(!w.view(1).visible);
}

#[test]
fn non_zero_return_code_clears_and_passes() {
    let mut w = World::new(shell());
    w.open(1, Some("editor"));
    w.activate(1);
    w.type_keys(1, "su3 ");
    assert!(!w.press(1, KeyInput::Mapped(keys::special(99, 0))));
    assert_eq!(w.view(1).marked, "");
    assert!(!w.view(1).visible);
    assert!(!w.enter(1, 0), "the core holds no composition either");
    w.type_keys(1, "cl3");
    assert_eq!(w.view(1).marked, "好");
}

#[test]
fn engine_that_cannot_be_built_passes_every_key() {
    let mut s = setup();
    s.resources = s.resources.join("missing");
    let shell = Shell::new(s, Prefs::memory(), &Defaults::upstream());
    assert!(!shell.has_engine());
    let mut w = World::new(shell);
    w.open(1, Some("editor"));
    w.activate(1);
    assert_eq!(w.type_keys(1, "su3"), vec![false; 3]);
    assert!(w.view(1).inserts.is_empty());
}

#[test]
fn profile_follows_the_app() {
    let mut w = World::new(shell());
    w.open(1, Some("chat.app"));
    w.open(2, Some("editor"));
    w.activate(1);
    w.type_keys(1, ROW284);
    w.enter(1, 0);
    w.activate(2);
    w.type_keys(2, ROW284);
    w.enter(2, 0);
    assert_eq!(w.view(1).text, ROW284_CHAT);
    assert_eq!(w.view(2).text, ROW284_FORMAL, "律水壺 needs trigram.sjt3 beside the model (Selftest.swift)");
}

#[test]
fn unknown_app_is_formal() {
    let mut w = World::new(shell());
    w.open(1, None);
    w.activate(1);
    w.type_keys(1, ROW284);
    w.enter(1, 0);
    assert_eq!(w.view(1).text, ROW284_FORMAL);
}

#[test]
fn owner_a_key_on_another_session_commits_to_the_owner() {
    let mut w = World::new(shell());
    w.open(1, Some("editor"));
    w.open(2, Some("editor"));
    w.activate(1);
    w.type_keys(1, "su3 ");
    assert!(w.view(1).visible);
    w.type_keys(2, "c");
    assert_eq!(w.view(1).text, "你");
    assert_eq!(w.view(1).marked, "");
    assert_eq!(w.view(2).marked, "ㄏ");
    assert!(w.view(2).inserts.is_empty());
    w.type_keys(2, "l3");
    w.enter(2, 0);
    assert_eq!(w.view(2).text, "好");
}

#[test]
fn owner_b_closed_owner_is_discarded() {
    let mut w = World::new(shell());
    w.open(1, Some("editor"));
    w.activate(1);
    w.type_keys(1, "su3 ");
    let mut out = Effects::new();
    w.shell.close_session(1, &mut out);
    w.run(out);
    assert!(!w.shell.composing());
    assert!(w.view(1).inserts.is_empty(), "discarded, not committed");
    w.open(2, Some("editor"));
    w.activate(2);
    w.type_keys(2, "cl3");
    w.enter(2, 0);
    assert_eq!(w.view(2).text, "好");
}

#[test]
fn owner_c_late_deactivate_of_a_non_owner_does_nothing() {
    let mut w = World::new(shell());
    w.open(1, Some("editor"));
    w.open(2, Some("editor"));
    w.activate(1);
    w.activate(2);
    w.type_keys(2, "su3 ");
    let mut out = Effects::new();
    w.shell.deactivate(1, &mut out);
    w.shell.commit_composition(1, &mut out);
    assert!(out.is_empty());
    assert!(w.view(2).visible);
}

#[test]
fn deactivate_commits_unless_secure() {
    let mut w = World::new(shell());
    w.open(1, Some("editor"));
    w.activate(1);
    w.type_keys(1, "su3 ");
    let mut out = Effects::new();
    w.shell.deactivate(1, &mut out);
    w.run(out);
    assert_eq!(w.view(1).text, "你");
    assert!(!w.view(1).visible);

    let mut w = World::new(shell());
    w.open(1, Some("editor"));
    w.shell.set_client(1, ClientInfo { app_id: Some("editor".into()), secure_input: true, left: None });
    w.activate(1);
    w.type_keys(1, "su3");
    let mut out = Effects::new();
    w.shell.deactivate(1, &mut out);
    w.run(out);
    assert_eq!(w.view(1).text, "", "secure input: discarded");
}

#[test]
fn stored_eten_layout_applies_and_menu_switch_stores() {
    let mut prefs = Prefs::memory();
    prefs.set_string(key::LAYOUT, "eten");
    let mut w = World::new(shell_with(prefs));
    w.open(1, Some("editor"));
    w.activate(1);
    assert_eq!(w.shell.layout(), InputMode::Eten);
    w.type_keys(1, "s");
    assert_eq!(w.view(1).marked, "ㄙ");
    w.perform(1, Action::Layout(InputMode::Standard));
    assert_eq!(w.view(1).text, "ㄙ", "the composition was committed before the rebuild");
    w.type_keys(1, "s");
    assert_eq!(w.view(1).marked, "ㄋ");
}

#[test]
fn menu_order_and_strings() {
    let mut w = World::new(shell());
    w.open(1, Some("editor"));
    let titles: Vec<String> = w.shell.menu(1).into_iter().map(|e: MenuEntry| e.title).collect();
    let date = titles.iter().find(|t| t.starts_with("資料更新至")).cloned().expect("the ACG manifest date line");
    assert_eq!(
        titles,
        vec![
            text::STANDARD, text::ETEN, text::DEMOTE, text::PREDICTION, text::ENGLISH, text::RERANK, text::ACG_PACK, &date,
            text::APP_CHAT, text::SETTINGS, text::CLEAR,
        ]
    );
    w.open(2, None);
    assert_eq!(w.shell.menu(2)[0].title, text::PAUSED_APP, "no app identity: learning paused (Learning.swift)");
}

#[test]
fn menu_toggles_store_and_survive_a_restart() {
    let dir = std::env::temp_dir().join(format!("shanjie-shell-prefs-{}", std::process::id()));
    let path = dir.join("settings.ini");
    {
        let mut w = World::new(shell_with(Prefs::open(path.clone())));
        w.open(1, Some("editor"));
        w.perform(1, Action::TogglePrediction);
        w.perform(1, Action::ToggleDemote);
        w.perform(1, Action::ToggleAcgPack);
        let mut out = Effects::new();
        w.shell.apply_rerank_level(RerankLevel::Aggressive, &mut out);
        w.shell.set_profile_mode(ProfileMode::Chat, &mut out);
    }
    let s = shell_with(Prefs::open(path));
    let st = s.settings();
    assert!(!st.prediction && !st.demote && !st.acg_pack);
    assert!(!st.rerank_enabled, "語意校正 greys out with the demotion table off");
    assert_eq!(st.rerank_level, RerankLevel::Aggressive);
    assert_eq!(st.profile_mode, ProfileMode::Chat);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn overlay_defaults_apply_only_to_never_chosen_keys() {
    let mut d = Defaults::upstream();
    d.prediction = false;
    let s = Shell::new(setup(), Prefs::memory(), &d);
    assert!(!s.settings().prediction);
    let mut prefs = Prefs::memory();
    prefs.set_bool(key::PREDICTION, true);
    let s = Shell::new(setup(), prefs, &d);
    assert!(s.settings().prediction, "a saved value wins over the overlay");
}

#[test]
fn toggle_app_chat_flips_and_stores_only_differences() {
    let mut w = World::new(shell());
    w.open(1, Some("editor"));
    w.activate(1);
    w.perform(1, Action::ToggleAppChat);
    assert_eq!(w.shell.settings().app_profiles, vec![("editor".to_string(), true)]);
    w.perform(1, Action::ToggleAppChat);
    assert!(w.shell.settings().app_profiles.is_empty(), "back to the built-in value: the entry goes");
}

#[test]
fn english_run_shows_the_hint_and_a_click_on_it_picks_nothing() {
    let mut w = World::new(shell());
    w.open(1, Some("editor"));
    w.activate(1);
    w.type_keys(1, "su3");
    assert_eq!(w.view(1).hint, None, "no run, no hint");
    w.press(1, ch('w', keys::SHIFT));
    w.type_keys(1, "indows");
    assert_eq!(w.view(1).marked, "你Windows");
    assert_eq!(w.view(1).run, (1, 7), "the run after the one Chinese character");
    assert_eq!(w.view(1).hint.as_deref(), Some(Shell::ENGLISH_RUN_HINT));
    let mut out = Effects::new();
    assert!(!w.shell.candidate_selected(1, 0, &mut out));
}

#[test]
fn clear_asks_first() {
    let mut w = World::new(shell());
    w.open(1, Some("editor"));
    w.perform(1, Action::Clear);
    assert_eq!(w.view(1).confirm, 1);
}

#[test]
fn left_context_is_the_trailing_han() {
    assert_eq!(shanjie_shell::shell::han_tail("我今天", false).as_deref(), Some("今天"));
    assert_eq!(shanjie_shell::shell::han_tail("abc", false), None);
    assert_eq!(shanjie_shell::shell::han_tail("天x", false), None);
    let long = "一".repeat(40);
    let t = shanjie_shell::shell::left_text(&long);
    assert!(t.truncated);
    assert_eq!(t.text.encode_utf16().count(), 32);
    let _ = LeftText::default();
}

/// Shell.swift's rerank queue (RerankTests.swift): one job runs at a time, only the latest waits, and a finished job is
/// applied only while its session still owns the composition.
#[test]
fn rerank_jobs_run_one_at_a_time_and_apply_to_the_owner() {
    use std::cell::RefCell;
    use std::rc::Rc;
    struct Held(Rc<RefCell<Vec<(u64, shanjie_shell::core::RerankJob)>>>);
    impl shanjie_shell::RerankRunner for Held {
        fn run(&self, ticket: u64, job: shanjie_shell::core::RerankJob) {
            self.0.borrow_mut().push((ticket, job));
        }
    }
    let jobs = Rc::new(RefCell::new(Vec::new()));
    let mut s = shell();
    s.set_rerank_runner(Box::new(Held(jobs.clone())));
    let mut w = World::new(s);
    w.open(1, Some("editor"));
    w.activate(1);
    w.type_keys(1, ROW284);
    let started = jobs.borrow().len();
    assert_eq!(started, 1, "one job runs; later ones wait (only the latest)");
    let (ticket, job) = jobs.borrow_mut().remove(0);
    let code = job.run();
    let mut out = Effects::new();
    w.shell.rerank_finished(ticket, job, code, &mut out);
    w.run(out);
    assert_eq!(jobs.borrow().len(), 1, "the waiting job starts when the running one finishes");
    w.enter(1, 0);
    assert!(!w.view(1).text.is_empty());
}
