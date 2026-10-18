//! The host's logic with the real core and data (`ports/data/assemble.sh build/ports/data`, or `SHANJIE_TEST_DATA`),
//! fake pipes and a fake candidate window. Runs on any platform.
use shanjie_host::hub::{Dialogs, Hub, Listener, Panel, View};
use shanjie_proto::{Client, Event, RawKey, Reply, Request, Text, MOD_SHIFT, PROTOCOL};
use shanjie_shell::prefs::{Defaults, Prefs};
use shanjie_shell::{Setup, Shell};
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

fn data() -> PathBuf {
    std::env::var_os("SHANJIE_TEST_DATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../build/ports/data"))
}

#[derive(Default)]
struct Seen {
    panel: Option<(View, Option<[i32; 4]>)>,
    events: Vec<Event>,
    clear_failed: u32,
}

type Shared = Rc<RefCell<Seen>>;

struct FakePanel(Shared);
impl Panel for FakePanel {
    fn show(&mut self, view: View, anchor: Option<[i32; 4]>) {
        self.0.borrow_mut().panel = Some((view, anchor));
    }
    fn hide(&mut self) {
        self.0.borrow_mut().panel = None;
    }
}
struct FakeDialogs(Shared);
impl Dialogs for FakeDialogs {
    fn clear_failed(&mut self) {
        self.0.borrow_mut().clear_failed += 1;
    }
}
struct FakeListener(Shared);
impl Listener for FakeListener {
    fn send(&mut self, e: &Event) -> bool {
        self.0.borrow_mut().events.push(e.clone());
        true
    }
}

fn hub() -> (Hub, Shared) {
    hub_learning_in(None)
}

fn hub_learning_in(learning_directory: Option<PathBuf>) -> (Hub, Shared) {
    let tables = shanjie_host::tables::tables();
    let setup = Setup {
        resources: data(),
        learning_directory,
        tables,
        shows_abbreviation: false,
        punctuation: None,
        names: HashMap::new(),
    };
    let shell = Shell::new(setup, Prefs::memory(), &Defaults::upstream());
    assert!(shell.has_engine(), "no engine: assemble the data into {}", data().display());
    let seen = Shared::default();
    (Hub::new(shell, Box::new(FakePanel(seen.clone())), Box::new(FakeDialogs(seen.clone()))), seen)
}

fn hello(h: &mut Hub, app: &str) -> u64 {
    match h.request(Request::Hello { protocol: PROTOCOL, app: Some(app.into()) }) {
        Reply::Session(id) => id,
        r => panic!("{r:?}"),
    }
}

const SCAN: [(char, u16); 47] = [
    ('1', 0x02), ('2', 0x03), ('3', 0x04), ('4', 0x05), ('5', 0x06), ('6', 0x07), ('7', 0x08), ('8', 0x09), ('9', 0x0A),
    ('0', 0x0B), ('-', 0x0C), ('=', 0x0D), ('q', 0x10), ('w', 0x11), ('e', 0x12), ('r', 0x13), ('t', 0x14), ('y', 0x15),
    ('u', 0x16), ('i', 0x17), ('o', 0x18), ('p', 0x19), ('[', 0x1A), (']', 0x1B), ('a', 0x1E), ('s', 0x1F), ('d', 0x20),
    ('f', 0x21), ('g', 0x22), ('h', 0x23), ('j', 0x24), ('k', 0x25), ('l', 0x26), (';', 0x27), ('\'', 0x28), ('`', 0x29),
    ('\\', 0x2B), ('z', 0x2C), ('x', 0x2D), ('c', 0x2E), ('v', 0x2F), ('b', 0x30), ('n', 0x31), ('m', 0x32), (',', 0x33),
    ('.', 0x34), ('/', 0x35),
];

fn raw(c: char) -> RawKey {
    if c == ' ' {
        return RawKey { vk: 0x20, scan: 0x39, extended: false, modifiers: 0 };
    }
    let lower = c.to_ascii_lowercase();
    let scan = SCAN.iter().find(|(k, _)| *k == lower).map(|(_, s)| *s).unwrap();
    RawKey { vk: lower.to_ascii_uppercase() as u16, scan, extended: false, modifiers: if c.is_ascii_uppercase() { MOD_SHIFT } else { 0 } }
}

const ENTER: RawKey = RawKey { vk: 0x0D, scan: 0x1C, extended: false, modifiers: 0 };

/// Types `keys` into session `id` and returns every text effect the replies carried.
fn type_keys(h: &mut Hub, id: u64, keys: &str) -> Vec<Text> {
    let mut out = Vec::new();
    for c in keys.chars() {
        out.extend(key(h, id, raw(c)).1);
    }
    out
}

fn key(h: &mut Hub, id: u64, k: RawKey) -> (bool, Vec<Text>) {
    match h.request(Request::Key { session: id, key: k, client: Client { caret: Some([10, 20, 11, 40]), ..Default::default() } }) {
        Reply::Key { handled, text } => (handled, text),
        r => panic!("{r:?}"),
    }
}

fn inserts(t: &[Text]) -> Vec<String> {
    t.iter().filter_map(|t| if let Text::Insert(s) = t { Some(s.clone()) } else { None }).collect()
}

const ROW284: &str = "tj6z;6vup 5j; xk7u ek7xm4gjo3cj6";

#[test]
fn nihao_then_enter() {
    let (mut h, seen) = hub();
    let id = hello(&mut h, "notepad.exe");
    let t = type_keys(&mut h, id, "su3cl3");
    assert!(matches!(t.last(), Some(Text::Marked { text, .. }) if text == "你好"));
    assert!(seen.borrow().panel.as_ref().is_some_and(|(_, a)| *a == Some([10, 20, 11, 40])), "the window goes under the caret");
    let (handled, t) = key(&mut h, id, ENTER);
    assert!(handled);
    assert_eq!(inserts(&t), vec!["你好"]);
    assert!(seen.borrow().panel.is_none());
}

#[test]
fn chat_apps_by_executable_name() {
    let (mut h, _) = hub();
    let chat = hello(&mut h, "discord.exe");
    let mut t = type_keys(&mut h, chat, ROW284);
    t.extend(key(&mut h, chat, ENTER).1);
    assert_eq!(inserts(&t), vec!["廚房新裝了一個率水壺"]);
    let formal = hello(&mut h, "winword.exe");
    let mut t = type_keys(&mut h, formal, ROW284);
    t.extend(key(&mut h, formal, ENTER).1);
    assert_eq!(inserts(&t), vec!["廚房新裝了一個律水壺"]);
}

#[test]
fn a_key_in_another_app_commits_to_the_owner_through_its_event_pipe() {
    let (mut h, seen) = hub();
    let a = hello(&mut h, "notepad.exe");
    let b = hello(&mut h, "notepad.exe");
    assert!(h.listen(a, Box::new(FakeListener(seen.clone()))));
    type_keys(&mut h, a, "su3");
    let t = type_keys(&mut h, b, "c");
    assert!(matches!(t.as_slice(), [Text::Marked { text, .. }] if text == "ㄏ"));
    let events = seen.borrow().events.clone();
    assert!(events.iter().any(|e| matches!(e, Event::Text(t) if inserts(t) == vec!["你"])), "{events:?}");
}

#[test]
fn a_click_picks_for_the_window_owner() {
    let (mut h, seen) = hub();
    let a = hello(&mut h, "notepad.exe");
    assert!(h.listen(a, Box::new(FakeListener(seen.clone()))));
    type_keys(&mut h, a, "su3 ");
    let second = match &seen.borrow().panel {
        Some((View::Candidates { cells, selected, .. }, _)) => {
            assert_eq!(*selected, 0);
            cells[1].text.clone()
        }
        p => panic!("{p:?}"),
    };
    h.clicked(1);
    let events = seen.borrow().events.clone();
    assert!(events.iter().any(|e| matches!(e, Event::Text(t) if t.iter().any(|t| matches!(t, Text::Marked { text, .. } if *text == second)))));
}

#[test]
fn menu_and_choices() {
    let (mut h, seen) = hub();
    let a = hello(&mut h, "notepad.exe");
    assert!(h.listen(a, Box::new(FakeListener(seen.clone()))));
    let Reply::Menu(menu) = h.request(Request::Menu { session: a, client: Client::default() }) else { panic!() };
    assert_eq!(menu[0].key, "layout.standard");
    assert!(menu[0].checked);
    assert!(menu.iter().any(|n| n.key == "appChat"));
    h.request(Request::Choose { session: a, key: "prediction".into(), client: Client::default() });
    assert!(!h.shell().settings().prediction);
    assert!(seen.borrow().events.contains(&Event::MenuChanged));
}

#[test]
fn closing_the_owner_hides_the_window() {
    let (mut h, seen) = hub();
    let a = hello(&mut h, "notepad.exe");
    type_keys(&mut h, a, "su3 ");
    assert!(seen.borrow().panel.is_some());
    h.request(Request::Close { session: a });
    assert!(seen.borrow().panel.is_none());
    assert!(matches!(key(&mut h, a, raw('a')), (false, t) if t.is_empty()), "a closed session passes keys");
}

#[test]
fn another_protocol_gets_no_session() {
    let (mut h, _) = hub();
    assert_eq!(h.request(Request::Hello { protocol: PROTOCOL + 1, app: None }), Reply::Session(0));
}

/// The learning store through the core's file code (on Windows, its shim: ports/docs/core-windows.md): a pick is
/// written to learning.tsv, and a new engine opens the file again.
#[test]
fn a_pick_is_written_to_the_learning_file() {
    let dir = std::env::temp_dir().join(format!("shanjie-host-learning-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let (mut h, seen) = hub_learning_in(Some(dir.clone()));
    let a = hello(&mut h, "notepad.exe");
    type_keys(&mut h, a, "su3 ");
    let second = match &seen.borrow().panel {
        Some((View::Candidates { cells, .. }, _)) => cells[1].text.clone(),
        p => panic!("{p:?}"),
    };
    let mut t = type_keys(&mut h, a, "2");
    t.extend(key(&mut h, a, ENTER).1);
    assert_eq!(inserts(&t).concat(), second);
    assert!(!h.shell().learning_unavailable());
    let file = std::fs::read_to_string(dir.join("learning.tsv")).expect("learning.tsv");
    assert!(file.starts_with("#shanjie-learning v1"), "{file}");
    assert!(file.contains(&second), "{file}");
    drop(h);
    let (h, _) = hub_learning_in(Some(dir.clone()));
    assert!(!h.shell().learning_unavailable(), "the file opens again");
    let _ = std::fs::remove_dir_all(&dir);
}
