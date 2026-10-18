//! The one thread that owns the shell, as the macOS input method's main thread owns `Shell` (the engine handle is used on
//! one thread only, s3a section 6). D-Bus handlers send it a `Msg` and wait for the reply; rerank results come back from
//! the worker the same way. Every reply carries the IBus signals to emit, per engine object, in the order the shell made
//! the calls.
//!
//! The IBus side of each `Effect` (what IMK does on macOS):
//! - `Insert` is CommitText. While the session handles FocusOut or Reset, the client has already committed the preedit
//!   itself (preedit mode COMMIT, below), so the shell's `Insert` of that same text is not sent again.
//! - `Marked` is UpdatePreeditText with a single underline, a double one over the English run (IBus has no thick
//!   underline), the cursor converted from UTF-16 units to characters. The preedit mode is COMMIT, so the client commits
//!   what is shown when it loses focus or resets (the macOS shell's `deactivate` and `commitComposition` commit too), and
//!   CLEAR in a password field, where `deactivate` discards (Shell.swift: never send the composition into a secure field).
//! - `ShowCandidates` is UpdateLookupTable: one page exactly as the core shows it, numbered by `panel::numbers`; the
//!   expanded grid shows its selected row (IBus's candidate window has no grid); vertical when the core says so.
//! - `ShowHint` is the auxiliary text; `HideCandidates` hides both.
use crate::props;
use crate::serial::{self, Property};
use shanjie_shell::core::RerankJob;
use shanjie_shell::keys::KeyInput;
use shanjie_shell::menu::dialog;
use shanjie_shell::panel;
use shanjie_shell::prefs::{Defaults, Prefs};
use shanjie_shell::{ClientInfo, Effect, Effects, SessionId, Setup, Shell};
use std::collections::HashMap;
use std::sync::mpsc;

/// IBusPreeditFocusMode.
const PREEDIT_CLEAR: u32 = 0;
const PREEDIT_COMMIT: u32 = 1;

/// IBusInputPurpose: PASSWORD and PIN are the platform's secure input.
const PURPOSE_PASSWORD: u32 = 8;
const PURPOSE_PIN: u32 = 9;

/// One signal for one engine object.
pub enum Signal {
    Commit(String),
    /// Text, cursor in characters, (type, value, start, end) spans in characters, visible, mode.
    Preedit(String, u32, Vec<(u32, u32, u32, u32)>, bool, u32),
    /// Candidates, labels, cursor, vertical, visible.
    Lookup(Vec<String>, Vec<String>, Option<u32>, bool, bool),
    Auxiliary(String, bool),
    Properties(Vec<Property>),
    RequireSurrounding,
}

pub type Out = Vec<(SessionId, Signal)>;

pub enum Msg {
    Open(SessionId),
    Close(SessionId),
    Key { id: SessionId, keyval: u32, keycode: u32, state: u32, reply: async_channel::Sender<(bool, Out)> },
    FocusIn { id: SessionId, client: Option<String>, reply: async_channel::Sender<Out> },
    FocusOut { id: SessionId, reply: async_channel::Sender<Out> },
    Reset { id: SessionId, reply: async_channel::Sender<Out> },
    Enable { id: SessionId, reply: async_channel::Sender<Out> },
    Disable { id: SessionId, reply: async_channel::Sender<Out> },
    Surrounding { id: SessionId, text: String, cursor: u32, anchor: u32 },
    ContentType { id: SessionId, purpose: u32 },
    Click { id: SessionId, index: u32, reply: async_channel::Sender<Out> },
    Page { id: SessionId, up: bool, reply: async_channel::Sender<Out> },
    Property { id: SessionId, name: String, state: u32, reply: async_channel::Sender<Out> },
    RerankDone { ticket: u64, job: RerankJob, code: i32 },
}

impl Msg {
    /// One line for `SHANJIE_TRACE`.
    fn describe(&self) -> String {
        match self {
            Msg::Open(id) => format!("open {id}"),
            Msg::Close(id) => format!("close {id}"),
            Msg::Key { id, keyval, keycode, state, .. } => format!("key {id} {keyval:#x} {keycode} {state:#x}"),
            Msg::FocusIn { id, client, .. } => format!("focus in {id} {client:?}"),
            Msg::FocusOut { id, .. } => format!("focus out {id}"),
            Msg::Reset { id, .. } => format!("reset {id}"),
            Msg::Enable { id, .. } => format!("enable {id}"),
            Msg::Disable { id, .. } => format!("disable {id}"),
            Msg::Surrounding { id, text, cursor, anchor } => format!("surrounding {id} {text:?} {cursor} {anchor}"),
            Msg::ContentType { id, purpose } => format!("content type {id} {purpose}"),
            Msg::Click { id, index, .. } => format!("click {id} {index}"),
            Msg::Page { id, up, .. } => format!("page {id} up={up}"),
            Msg::Property { id, name, state, .. } => format!("property {id} {name} {state}"),
            Msg::RerankDone { ticket, code, .. } => format!("rerank done {ticket} {code}"),
        }
    }
}

#[derive(Default)]
struct Ctx {
    app: Option<String>,
    secure: bool,
    left: Option<String>,
    /// The last preedit went out with mode COMMIT.
    commit_mode: bool,
    /// Index of the first candidate the lookup table shows, within the core's output (the grid's selected row).
    shown_offset: u32,
}

pub struct Hub {
    shell: Shell,
    ctx: HashMap<SessionId, Ctx>,
    focused: Option<SessionId>,
    async_out: async_channel::Sender<Out>,
}

/// The program part of an IBus client name: `gtk3-im:gedit` is `gedit`; a name without a colon is used whole.
pub fn app_id(client: &str) -> Option<String> {
    let id = client.rsplit(':').next().unwrap_or(client).trim();
    shanjie_shell::prefs::valid_id(id).then(|| id.to_string())
}

/// The text before `cursor` (characters) as the left context request (Learning.swift: the selection's start).
fn before(text: &str, cursor: u32, anchor: u32) -> String {
    let at = cursor.min(anchor) as usize;
    text.chars().take(at).collect()
}

fn utf16_to_chars(s: &str, utf16: u32) -> u32 {
    let mut units = 0u32;
    let mut chars = 0u32;
    for c in s.chars() {
        if units >= utf16 {
            break;
        }
        units += c.len_utf16() as u32;
        chars += 1;
    }
    chars
}

struct Runner(mpsc::Sender<(u64, RerankJob)>);

impl shanjie_shell::RerankRunner for Runner {
    fn run(&self, ticket: u64, job: RerankJob) {
        let _ = self.0.send((ticket, job));
    }
}

/// Starts the shell thread and the rerank worker. `async_out` receives the signals of work no D-Bus call waits for
/// (rerank results).
pub fn start(async_out: async_channel::Sender<Out>) -> mpsc::Sender<Msg> {
    let (tx, rx) = mpsc::channel::<Msg>();
    let back = tx.clone();
    std::thread::Builder::new()
        .name("shanjie-shell".into())
        .spawn(move || {
            let data = crate::paths::data();
            let (defaults, problems) = Defaults::load(&data.join("defaults.ini"));
            for p in problems {
                eprintln!("ibus-engine-shanjie: {p}");
            }
            let prefs = crate::paths::settings().map(Prefs::open).unwrap_or_else(Prefs::memory);
            let setup = Setup {
                resources: data,
                learning_directory: crate::paths::learning(),
                tables: crate::tables::tables(),
                shows_abbreviation: false,
                punctuation: None,
                names: HashMap::new(),
            };
            let mut shell = Shell::new(setup, prefs, &defaults);
            if !shell.has_engine() {
                eprintln!("ibus-engine-shanjie: the core did not start; every key passes");
            }
            let (jobs_tx, jobs_rx) = mpsc::channel::<(u64, RerankJob)>();
            std::thread::Builder::new()
                .name("shanjie-rerank".into())
                .spawn(move || {
                    for (ticket, job) in jobs_rx {
                        let code = job.run();
                        if back.send(Msg::RerankDone { ticket, job, code }).is_err() {
                            break;
                        }
                    }
                })
                .expect("rerank thread");
            shell.set_rerank_runner(Box::new(Runner(jobs_tx)));
            let mut hub = Hub { shell, ctx: HashMap::new(), focused: None, async_out };
            let trace = std::env::var_os("SHANJIE_TRACE").is_some();
            if trace {
                eprintln!("ibus-engine-shanjie: data {}, settings {:?}, learning {:?}", crate::paths::data().display(), crate::paths::settings(), crate::paths::learning());
            }
            for msg in rx {
                if trace {
                    eprintln!("ibus-engine-shanjie: {}", msg.describe());
                }
                hub.handle(msg);
            }
        })
        .expect("shell thread");
    tx
}

impl Hub {
    fn info(&self, id: SessionId) -> ClientInfo {
        let c = self.ctx.get(&id);
        ClientInfo {
            app_id: c.and_then(|c| c.app.clone()),
            secure_input: c.is_some_and(|c| c.secure),
            left: c.and_then(|c| c.left.as_deref()).map(shanjie_shell::shell::left_text),
        }
    }

    fn sync(&mut self, id: SessionId) {
        let info = self.info(id);
        self.shell.set_client(id, info);
    }

    fn handle(&mut self, msg: Msg) {
        match msg {
            Msg::Open(id) => {
                self.ctx.insert(id, Ctx::default());
                let info = self.info(id);
                self.shell.open_session(id, info);
            }
            Msg::Close(id) => {
                let mut fx = Effects::new();
                self.shell.close_session(id, &mut fx);
                self.ctx.remove(&id);
                if self.focused == Some(id) {
                    self.focused = None;
                }
                let out = self.translate(fx, None);
                let _ = self.async_out.send_blocking(out);
            }
            Msg::Key { id, keyval, keycode, state, reply } => {
                self.sync(id);
                let (handled, out) = match crate::keys::translate(keyval, keycode, state) {
                    None => (false, Out::new()),
                    Some(input) => {
                        let mut fx = Effects::new();
                        let handled = self.shell.handle(id, input, &mut fx);
                        (handled, self.translate(fx, None))
                    }
                };
                let _ = reply.send_blocking((handled, out));
            }
            Msg::FocusIn { id, client, reply } => {
                if let Some(c) = self.ctx.get_mut(&id) {
                    if let Some(client) = client {
                        c.app = app_id(&client);
                    }
                }
                self.focused = Some(id);
                self.sync(id);
                let mut fx = Effects::new();
                self.shell.activate(id, &mut fx);
                let mut out = vec![(id, Signal::RequireSurrounding)];
                out.extend(self.translate(fx, None));
                out.push((id, Signal::Properties(self.props(id))));
                let _ = reply.send_blocking(out);
            }
            Msg::FocusOut { id, reply } | Msg::Disable { id, reply } => {
                self.sync(id);
                let mut fx = Effects::new();
                self.shell.deactivate(id, &mut fx);
                let out = self.translate(fx, Some(id));
                let _ = reply.send_blocking(out);
            }
            Msg::Reset { id, reply } => {
                self.sync(id);
                let mut fx = Effects::new();
                self.shell.commit_composition(id, &mut fx);
                let out = self.translate(fx, Some(id));
                let _ = reply.send_blocking(out);
            }
            Msg::Enable { id, reply } => {
                let _ = reply.send_blocking(vec![(id, Signal::RequireSurrounding), (id, Signal::Properties(self.props(id)))]);
            }
            Msg::Surrounding { id, text, cursor, anchor } => {
                if let Some(c) = self.ctx.get_mut(&id) {
                    c.left = Some(before(&text, cursor, anchor));
                }
            }
            Msg::ContentType { id, purpose } => {
                if let Some(c) = self.ctx.get_mut(&id) {
                    c.secure = purpose == PURPOSE_PASSWORD || purpose == PURPOSE_PIN;
                }
            }
            Msg::Click { id, index, reply } => {
                self.sync(id);
                let offset = self.ctx.get(&id).map_or(0, |c| c.shown_offset);
                let mut fx = Effects::new();
                self.shell.candidate_selected(id, index + offset, &mut fx);
                let out = self.translate(fx, None);
                let _ = reply.send_blocking(out);
            }
            Msg::Page { id, up, reply } => {
                // The candidate window's own page buttons: Page Up / Down without modifiers.
                self.sync(id);
                let mut fx = Effects::new();
                self.shell.handle(id, KeyInput::Page { up, modifiers: 0 }, &mut fx);
                let out = self.translate(fx, None);
                let _ = reply.send_blocking(out);
            }
            Msg::Property { id, name, state, reply } => {
                self.sync(id);
                let mut fx = Effects::new();
                props::activate(&mut self.shell, id, &name, state, &mut fx);
                let mut out = self.translate(fx, None);
                out.push((id, Signal::Properties(self.props(id))));
                let _ = reply.send_blocking(out);
            }
            Msg::RerankDone { ticket, job, code } => {
                let mut fx = Effects::new();
                self.shell.rerank_finished(ticket, job, code, &mut fx);
                let out = self.translate(fx, None);
                if !out.is_empty() {
                    let _ = self.async_out.send_blocking(out);
                }
            }
        }
    }

    fn props(&self, id: SessionId) -> Vec<Property> {
        props::build(&self.shell, id)
    }

    /// The effects as IBus signals. `losing`: the session handling FocusOut / Reset (its preedit the client commits itself).
    fn translate(&mut self, fx: Effects, losing: Option<SessionId>) -> Out {
        let mut out = Out::new();
        let mut settings_changed = false;
        for (id, e) in fx {
            match e {
                Effect::Insert(text) => {
                    let skip = losing == Some(id) && self.ctx.get(&id).is_some_and(|c| c.commit_mode);
                    if !skip {
                        out.push((id, Signal::Commit(text)));
                    }
                }
                Effect::Marked { text, cursor_utf16, run_start, run_len } => {
                    let secure = self.ctx.get(&id).is_some_and(|c| c.secure);
                    let chars = text.chars().count() as u32;
                    let cursor = utf16_to_chars(&text, cursor_utf16);
                    let mut spans = Vec::new();
                    let rs = utf16_to_chars(&text, run_start);
                    let re = utf16_to_chars(&text, run_start + run_len);
                    if run_len > 0 && re > rs {
                        if rs > 0 {
                            spans.push((serial::ATTR_UNDERLINE, serial::UNDERLINE_SINGLE, 0, rs));
                        }
                        spans.push((serial::ATTR_UNDERLINE, serial::UNDERLINE_DOUBLE, rs, re));
                        if re < chars {
                            spans.push((serial::ATTR_UNDERLINE, serial::UNDERLINE_SINGLE, re, chars));
                        }
                    } else {
                        spans.push((serial::ATTR_UNDERLINE, serial::UNDERLINE_SINGLE, 0, chars));
                    }
                    let mode = if secure { PREEDIT_CLEAR } else { PREEDIT_COMMIT };
                    if let Some(c) = self.ctx.get_mut(&id) {
                        c.commit_mode = mode == PREEDIT_COMMIT;
                    }
                    out.push((id, Signal::Preedit(text, cursor, spans, true, mode)));
                }
                Effect::ClearMarked => {
                    if let Some(c) = self.ctx.get_mut(&id) {
                        c.commit_mode = false;
                    }
                    out.push((id, Signal::Preedit(String::new(), 0, Vec::new(), false, PREEDIT_CLEAR)));
                }
                Effect::ShowCandidates { candidates, selected, columns, vertical, .. } => {
                    let numbers = panel::numbers(candidates.len(), selected, columns);
                    let (start, len) = panel::selected_row(candidates.len(), selected, columns);
                    let shown: Vec<String> = candidates[start..start + len].to_vec();
                    // Every cell gets a label: gnome-shell numbers a cell without one itself.
                    let labels = numbers[start..start + len].iter().map(|n| n.clone().unwrap_or_else(|| " ".into())).collect();
                    let cursor = (selected >= 0).then(|| selected as u32 - start as u32);
                    if let Some(c) = self.ctx.get_mut(&id) {
                        c.shown_offset = start as u32;
                    }
                    out.push((id, Signal::Auxiliary(String::new(), false)));
                    out.push((id, Signal::Lookup(shown, labels, cursor, vertical != 0, true)));
                }
                Effect::ShowHint { text, .. } => {
                    out.push((id, Signal::Lookup(Vec::new(), Vec::new(), None, false, false)));
                    out.push((id, Signal::Auxiliary(text, true)));
                }
                Effect::HideCandidates => {
                    if id != shanjie_shell::shell::NO_SESSION {
                        out.push((id, Signal::Lookup(Vec::new(), Vec::new(), None, false, false)));
                        out.push((id, Signal::Auxiliary(String::new(), false)));
                    }
                }
                Effect::ClearFailed => notify(dialog::FAILED_TITLE, dialog::FAILED_MESSAGE),
                // The confirmation is the 清除 item of the menu's own submenu (props.rs); the settings live in the menu.
                Effect::ConfirmClear | Effect::OpenSettings => {}
                Effect::SettingsChanged => settings_changed = true,
            }
        }
        if settings_changed {
            if let Some(f) = self.focused {
                out.push((f, Signal::Properties(self.props(f))));
            }
        }
        out
    }
}

/// The clear's failure window (Learning.swift `clearFailed`, title, message and 好): a desktop notification when
/// `notify-send` exists.
fn notify(title: &str, body: &str) {
    let _ = std::process::Command::new("notify-send").args(["--app-name=善解", title, body]).spawn();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_ids_from_client_names() {
        assert_eq!(app_id("gtk3-im:gedit").as_deref(), Some("gedit"));
        assert_eq!(app_id("gnome-shell").as_deref(), Some("gnome-shell"));
        assert_eq!(app_id(""), None);
    }

    #[test]
    fn cursor_units() {
        assert_eq!(utf16_to_chars("你好", 2), 2);
        assert_eq!(utf16_to_chars("𠀀a", 2), 1);
        assert_eq!(before("你好嗎", 2, 3), "你好");
    }
}
