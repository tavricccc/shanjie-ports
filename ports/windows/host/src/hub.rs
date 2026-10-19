//! The host's side of every DLL call: the shell, one session per DLL instance (one per app thread that has the text
//! service active), and the shell's effects sent where macOS sends them:
//! - `Insert` / `Marked` / `ClearMarked` (IMK client calls) go to the session's DLL: in the reply when that DLL is the
//!   caller, else as an `Event` on its event pipe (the composition owner of another app, a click, a rerank result);
//! - `ShowCandidates` / `ShowHint` / `HideCandidates` (the panel) go to the host's own candidate window, under the caret
//!   the session last reported;
//! - `ClearFailed` is the failure window; `SettingsChanged` tells every DLL its cached menu is stale.
//! `ConfirmClear` and `OpenSettings` need nothing here: the clear's confirmation and the settings live in the menu
//! (`shanjie_shell::menu_tree`).
use crate::keys;
use shanjie_proto::{Client, Event, Node, NodeKind, Reply, Request, Text, PROTOCOL};
use shanjie_shell::core::RerankJob;
use shanjie_shell::menu_tree::{self, Kind};
use shanjie_shell::{panel, ClientInfo, Effect, Effects, SessionId, Shell};
use std::collections::HashMap;

/// One cell of the candidate window.
#[derive(Clone, Debug, PartialEq)]
pub struct Cell {
    pub text: String,
    pub note: Option<String>,
    /// The number slot (`panel::numbers`).
    pub number: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum View {
    /// `columns` > 0: the expanded grid, `columns` cells per row.
    Candidates { cells: Vec<Cell>, selected: i32, columns: u32, vertical: bool },
    Hint { text: String, vertical: bool },
}

/// The candidate window.
pub trait Panel {
    /// `anchor`: the caret rectangle on screen (left, top, right, bottom) the window goes under.
    fn show(&mut self, view: View, anchor: Option<[i32; 4]>);
    fn hide(&mut self);
}

/// A DLL's event pipe. `false`: the pipe is gone.
pub trait Listener {
    fn send(&mut self, event: &Event) -> bool;
}

/// The windows the host opens itself.
pub trait Dialogs {
    /// Learning.swift `clearFailed`.
    fn clear_failed(&mut self);
}

#[derive(Default)]
struct Ctx {
    app: Option<String>,
    client: Client,
    listener: Option<Box<dyn Listener>>,
}

pub struct Hub {
    shell: Shell,
    sessions: HashMap<SessionId, Ctx>,
    next: SessionId,
    panel: Box<dyn Panel>,
    dialogs: Box<dyn Dialogs>,
    /// The session the candidate window shows for: a click goes to it.
    panel_session: Option<SessionId>,
}

fn node(n: menu_tree::Node) -> Node {
    Node {
        key: n.key,
        kind: match n.kind {
            Kind::Item => NodeKind::Item,
            Kind::Toggle => NodeKind::Toggle,
            Kind::Radio => NodeKind::Radio,
            Kind::Submenu => NodeKind::Submenu,
        },
        label: n.label,
        checked: n.checked,
        enabled: n.enabled,
        children: n.children.into_iter().map(node).collect(),
    }
}

impl Hub {
    pub fn new(shell: Shell, panel: Box<dyn Panel>, dialogs: Box<dyn Dialogs>) -> Hub {
        Hub { shell, sessions: HashMap::new(), next: 1, panel, dialogs, panel_session: None }
    }

    pub fn shell(&self) -> &Shell {
        &self.shell
    }

    fn info(&self, id: SessionId) -> ClientInfo {
        let c = self.sessions.get(&id);
        ClientInfo {
            app_id: c.and_then(|c| c.app.clone()),
            secure_input: c.is_some_and(|c| c.client.secure),
            left: c.and_then(|c| c.client.left.as_ref()).map(|(text, truncated)| shanjie_shell::LeftText { text: text.clone(), truncated: *truncated }),
        }
    }

    /// What the DLL read before this call becomes the session's client state.
    fn sync(&mut self, id: SessionId, client: Client) -> bool {
        let Some(c) = self.sessions.get_mut(&id) else { return false };
        c.client = client;
        let info = self.info(id);
        self.shell.set_client(id, info);
        true
    }

    pub fn listen(&mut self, session: SessionId, listener: Box<dyn Listener>) -> bool {
        match self.sessions.get_mut(&session) {
            Some(c) => {
                c.listener = Some(listener);
                true
            }
            None => false,
        }
    }

    /// One call. `Listen` is the transport's (`listen`); it is answered with `Done` here.
    pub fn request(&mut self, r: Request) -> Reply {
        match r {
            Request::Hello { protocol, app } => {
                if protocol != PROTOCOL {
                    return Reply::Session(0);
                }
                let id = self.next;
                self.next += 1;
                let app = app.filter(|a| shanjie_shell::prefs::valid_id(a));
                self.sessions.insert(id, Ctx { app, ..Default::default() });
                let info = self.info(id);
                self.shell.open_session(id, info);
                Reply::Session(id)
            }
            Request::Listen { .. } => Reply::Done,
            Request::Key { session, key, client } => {
                if !self.sync(session, client) {
                    return Reply::Key { handled: false, text: Vec::new() };
                }
                match keys::translate(key) {
                    None => Reply::Key { handled: false, text: Vec::new() },
                    Some(input) => {
                        let mut fx = Effects::new();
                        let handled = self.shell.handle(session, input, &mut fx);
                        Reply::Key { handled, text: self.route(fx, Some(session)) }
                    }
                }
            }
            Request::Activate { session, client } => self.call(session, client, |s, id, fx| s.activate(id, fx)),
            Request::Deactivate { session, client } => self.call(session, client, |s, id, fx| s.deactivate(id, fx)),
            Request::Commit { session, client } => self.call(session, client, |s, id, fx| s.commit_composition(id, fx)),
            Request::Menu { session, client } => {
                if !self.sync(session, client) {
                    return Reply::Menu(Vec::new());
                }
                Reply::Menu(menu_tree::build(&self.shell, session).into_iter().map(node).collect())
            }
            Request::Choose { session, key, client } => {
                self.call(session, client, move |s, id, fx| menu_tree::activate(s, id, &key, None, fx))
            }
            Request::Close { session } => {
                self.close(session);
                Reply::Done
            }
            // No client state comes with it: a press is read outside an edit session.
            Request::Modifier { session, key, down } => {
                if !self.sessions.contains_key(&session) {
                    return Reply::Text(Vec::new());
                }
                let mut fx = Effects::new();
                if let Some(m) = keys::modifier(key) {
                    self.shell.modifier(session, m, down, &mut fx);
                }
                Reply::Text(self.route(fx, Some(session)))
            }
        }
    }

    fn call(&mut self, id: SessionId, client: Client, f: impl FnOnce(&mut Shell, SessionId, &mut Effects)) -> Reply {
        if !self.sync(id, client) {
            return Reply::Text(Vec::new());
        }
        let mut fx = Effects::new();
        f(&mut self.shell, id, &mut fx);
        Reply::Text(self.route(fx, Some(id)))
    }

    /// The DLL went away (a Close, or its call pipe broke).
    pub fn close(&mut self, id: SessionId) {
        if !self.sessions.contains_key(&id) {
            return;
        }
        let mut fx = Effects::new();
        self.shell.close_session(id, &mut fx);
        let _ = self.route(fx, Some(id));
        self.sessions.remove(&id);
        if self.panel_session == Some(id) {
            self.panel_session = None;
            self.panel.hide();
        }
    }

    /// A click on cell `index` of the candidate window (`candidateSelected`).
    pub fn clicked(&mut self, index: u32) {
        let Some(id) = self.panel_session else { return };
        let mut fx = Effects::new();
        self.shell.candidate_selected(id, index, &mut fx);
        let _ = self.route(fx, None);
    }

    pub fn rerank_finished(&mut self, ticket: u64, job: RerankJob, code: i32) {
        let mut fx = Effects::new();
        self.shell.rerank_finished(ticket, job, code, &mut fx);
        let _ = self.route(fx, None);
    }

    /// Sends each effect where it goes; returns the caller's text effects, in order.
    fn route(&mut self, fx: Effects, caller: Option<SessionId>) -> Vec<Text> {
        let mut mine = Vec::new();
        let mut others: Vec<(SessionId, Vec<Text>)> = Vec::new();
        let mut settings_changed = false;
        for (id, e) in fx {
            let text = match e {
                Effect::Insert(s) => Some(Text::Insert(s)),
                Effect::Marked { text, cursor_utf16, run_start, run_len } => {
                    Some(Text::Marked { text, cursor: cursor_utf16, run_start, run_len })
                }
                Effect::ClearMarked => Some(Text::ClearMarked),
                Effect::ShowCandidates { candidates, notes, selected, columns, vertical, .. } => {
                    let numbers = panel::numbers(candidates.len(), selected, columns);
                    let cells = candidates
                        .into_iter()
                        .zip(numbers)
                        .enumerate()
                        .map(|(i, (text, number))| Cell { text, note: notes.get(i).cloned().flatten(), number })
                        .collect();
                    self.panel_session = Some(id);
                    let anchor = self.sessions.get(&id).and_then(|c| c.client.caret);
                    self.panel.show(View::Candidates { cells, selected, columns, vertical: vertical != 0 }, anchor);
                    None
                }
                Effect::ShowHint { text, vertical, .. } => {
                    self.panel_session = Some(id);
                    let anchor = self.sessions.get(&id).and_then(|c| c.client.caret);
                    self.panel.show(View::Hint { text, vertical: vertical != 0 }, anchor);
                    None
                }
                Effect::HideCandidates => {
                    self.panel_session = None;
                    self.panel.hide();
                    None
                }
                Effect::ClearFailed => {
                    self.dialogs.clear_failed();
                    None
                }
                Effect::SettingsChanged => {
                    settings_changed = true;
                    None
                }
                Effect::ConfirmClear | Effect::OpenSettings => None,
            };
            if let Some(t) = text {
                if Some(id) == caller {
                    mine.push(t);
                } else {
                    match others.iter_mut().find(|(o, _)| *o == id) {
                        Some((_, v)) => v.push(t),
                        None => others.push((id, vec![t])),
                    }
                }
            }
        }
        for (id, texts) in others {
            self.send(id, &Event::Text(texts));
        }
        if settings_changed {
            let ids: Vec<SessionId> = self.sessions.keys().copied().collect();
            for id in ids {
                self.send(id, &Event::MenuChanged);
            }
        }
        mine
    }

    fn send(&mut self, id: SessionId, e: &Event) {
        if let Some(c) = self.sessions.get_mut(&id) {
            if let Some(l) = c.listener.as_mut() {
                if !l.send(e) {
                    c.listener = None;
                }
            }
        }
    }
}
