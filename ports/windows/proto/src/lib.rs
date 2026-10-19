//! What the TSF DLL (one per app thread) and shanjie-host.exe (one per user) say to each other over named pipes.
//!
//! The split mirrors macOS: there one input method process owns the shell and the engine and every app is a client of
//! it (IMK). Here the host is that process; the DLL in each app only reads the document (left context, password field,
//! caret position), writes the composition, and forwards keys and menu choices.
//!
//! Two connections per DLL instance, both to `pipe_name`:
//! - the call pipe: `Request` -> `Reply`, one at a time, synchronous (a key is answered before TSF returns);
//! - the event pipe: opened with `Request::Listen`, after which the host writes `Event`s (text effects of work no call
//!   waits for: a click in the candidate window, a rerank result, a menu choice applied to another session).
//!
//! Framing: a little-endian u32 length, then the message; strings are u32 length + UTF-8.
use std::io::{self, Read, Write};

pub const PROTOCOL: u32 = 2;

/// The host's pipe for one user: the user's SID keeps two users of one machine apart (the DACL does the rest).
pub fn pipe_name(user_sid: &str) -> String {
    format!(r"\\.\pipe\ShanjiePorts-{PROTOCOL}-{user_sid}")
}

/// The single-instance mutex of the host, per user.
pub fn mutex_name(user_sid: &str) -> String {
    format!(r"Local\ShanjiePortsHost-{user_sid}")
}

// MARK: keys

/// Modifier bits of `RawKey::modifiers`, in the shell's order (KeyMap.swift): SHIFT, CONTROL, OPTION (Alt), COMMAND
/// (the Windows key), CAPSLOCK (toggled on).
pub const MOD_SHIFT: u8 = 1 << 0;
pub const MOD_CONTROL: u8 = 1 << 1;
pub const MOD_ALT: u8 = 1 << 2;
pub const MOD_WIN: u8 = 1 << 3;
pub const MOD_CAPSLOCK: u8 = 1 << 4;

/// A key-down as TSF reports it: the virtual key, the scan code (bits 16-23 of lParam) and the extended flag (bit 24).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RawKey {
    pub vk: u16,
    pub scan: u16,
    pub extended: bool,
    pub modifiers: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Special {
    Enter,
    Space,
    Backspace,
    Delete,
    Escape,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    Tab,
}

/// KeyMap.swift's tables by physical position (scan code set 1, which TSF reports whatever the keyboard layout).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    /// A key that only changes modifiers: on macOS it is no key-down at all (flagsChanged), so it passes untouched.
    Modifier,
    /// An ANSI keycap, unshifted.
    Keycap(char),
    Special(Special),
    /// Page Up / Page Down: the shell sends them only while the vertical window is open.
    Page { up: bool },
    /// Outside the tables (keypad digits, function keys, Insert, ...).
    Other,
}

/// VK_SHIFT, VK_CONTROL, VK_MENU, VK_CAPITAL, VK_LWIN, VK_RWIN, VK_LSHIFT..VK_RMENU, VK_NUMLOCK, VK_SCROLL: the keys
/// the Linux port also treats as modifier-only (ports/linux/src/keys.rs).
pub fn is_modifier_vk(vk: u16) -> bool {
    matches!(vk, 0x10 | 0x11 | 0x12 | 0x14 | 0x5B | 0x5C | 0xA0..=0xA5 | 0x90 | 0x91)
}

const KEYCAPS: [(u16, char); 47] = [
    (0x02, '1'), (0x03, '2'), (0x04, '3'), (0x05, '4'), (0x06, '5'), (0x07, '6'), (0x08, '7'), (0x09, '8'), (0x0A, '9'),
    (0x0B, '0'), (0x0C, '-'), (0x0D, '='), (0x10, 'q'), (0x11, 'w'), (0x12, 'e'), (0x13, 'r'), (0x14, 't'), (0x15, 'y'),
    (0x16, 'u'), (0x17, 'i'), (0x18, 'o'), (0x19, 'p'), (0x1A, '['), (0x1B, ']'), (0x1E, 'a'), (0x1F, 's'), (0x20, 'd'),
    (0x21, 'f'), (0x22, 'g'), (0x23, 'h'), (0x24, 'j'), (0x25, 'k'), (0x26, 'l'), (0x27, ';'), (0x28, '\''), (0x29, '`'),
    (0x2B, '\\'), (0x2C, 'z'), (0x2D, 'x'), (0x2E, 'c'), (0x2F, 'v'), (0x30, 'b'), (0x31, 'n'), (0x32, 'm'), (0x33, ','),
    (0x34, '.'), (0x35, '/'),
];

pub fn classify(k: RawKey) -> Class {
    if is_modifier_vk(k.vk) {
        return Class::Modifier;
    }
    let s = k.scan;
    if k.extended {
        return match s {
            0x1C => Class::Special(Special::Enter), // keypad Enter (KeyMap.swift: kVK_ANSI_KeypadEnter)
            0x53 => Class::Special(Special::Delete),
            0x4B => Class::Special(Special::Left),
            0x4D => Class::Special(Special::Right),
            0x48 => Class::Special(Special::Up),
            0x50 => Class::Special(Special::Down),
            0x47 => Class::Special(Special::Home),
            0x4F => Class::Special(Special::End),
            0x49 => Class::Page { up: true },
            0x51 => Class::Page { up: false },
            _ => Class::Other,
        };
    }
    match s {
        0x1C => Class::Special(Special::Enter),
        0x39 => Class::Special(Special::Space),
        0x0E => Class::Special(Special::Backspace),
        0x01 => Class::Special(Special::Escape),
        0x0F => Class::Special(Special::Tab),
        _ => KEYCAPS.iter().find(|(c, _)| *c == s).map_or(Class::Other, |(_, ch)| Class::Keycap(*ch)),
    }
}

pub fn keycaps() -> impl Iterator<Item = char> {
    KEYCAPS.iter().map(|(_, c)| *c)
}

// MARK: messages

/// What the DLL read from the document before the call.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Client {
    /// The field is a password or PIN field (its input scope).
    pub secure: bool,
    /// Up to 32 UTF-16 units before the selection, and whether the read started after the document's start.
    pub left: Option<(String, bool)>,
    /// The caret (or the composition's start) on screen, for the candidate window: left, top, right, bottom.
    pub caret: Option<[i32; 4]>,
}

/// The shell's client calls (`Effect::Insert`, `Marked`, `ClearMarked`); the candidate window is the host's own.
#[derive(Clone, Debug, PartialEq)]
pub enum Text {
    Insert(String),
    /// Text, caret, English run start and length, all in UTF-16 units.
    Marked { text: String, cursor: u32, run_start: u32, run_len: u32 },
    ClearMarked,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeKind {
    Item,
    Toggle,
    Radio,
    Submenu,
}

/// One menu node (`shanjie_shell::menu_tree::Node`).
#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    pub key: String,
    pub kind: NodeKind,
    pub label: String,
    pub checked: bool,
    pub enabled: bool,
    pub children: Vec<Node>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Request {
    /// The first call: the app's identity (its executable's file name, lower case). Answered with `Reply::Session`.
    Hello { protocol: u32, app: Option<String> },
    /// Turns this connection into the event pipe of `session`. No reply; `Event`s follow.
    Listen { session: u64 },
    /// A key-down. Answered with `Reply::Key`.
    Key { session: u64, key: RawKey, client: Client },
    /// The document got the focus (`activateServer`).
    Activate { session: u64, client: Client },
    /// The document lost the focus (`deactivateServer`).
    Deactivate { session: u64, client: Client },
    /// The app ended the composition itself (`commitComposition`); the DLL drops the `Insert` of text already there.
    Commit { session: u64, client: Client },
    /// The menu, for the language bar button.
    Menu { session: u64, client: Client },
    /// A menu node was chosen.
    Choose { session: u64, key: String, client: Client },
    /// The DLL instance is going away.
    Close { session: u64 },
    /// A modifier key alone, pressed or released (the Windows habit 單按 Shift). Sent outside an edit session for a
    /// press, inside one for a release, which may commit the composition. Answered with `Reply::Text`.
    Modifier { session: u64, key: RawKey, down: bool },
}

#[derive(Clone, Debug, PartialEq)]
pub enum Reply {
    Session(u64),
    Key { handled: bool, text: Vec<Text> },
    Text(Vec<Text>),
    Menu(Vec<Node>),
    Done,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    Text(Vec<Text>),
    /// The settings changed: a menu the DLL caches is stale.
    MenuChanged,
}

// MARK: encoding

#[derive(Default)]
pub struct Enc(Vec<u8>);

impl Enc {
    fn u8(&mut self, v: u8) {
        self.0.push(v);
    }
    fn u16(&mut self, v: u16) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn u32(&mut self, v: u32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn i32(&mut self, v: i32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn u64(&mut self, v: u64) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn bool(&mut self, v: bool) {
        self.u8(v as u8);
    }
    fn str(&mut self, v: &str) {
        self.u32(v.len() as u32);
        self.0.extend_from_slice(v.as_bytes());
    }
    fn opt_str(&mut self, v: &Option<String>) {
        match v {
            Some(s) => {
                self.bool(true);
                self.str(s);
            }
            None => self.bool(false),
        }
    }
    fn client(&mut self, c: &Client) {
        self.bool(c.secure);
        match &c.left {
            Some((t, truncated)) => {
                self.bool(true);
                self.str(t);
                self.bool(*truncated);
            }
            None => self.bool(false),
        }
        match c.caret {
            Some(r) => {
                self.bool(true);
                r.iter().for_each(|v| self.i32(*v));
            }
            None => self.bool(false),
        }
    }
    fn texts(&mut self, t: &[Text]) {
        self.u32(t.len() as u32);
        for e in t {
            match e {
                Text::Insert(s) => {
                    self.u8(0);
                    self.str(s);
                }
                Text::Marked { text, cursor, run_start, run_len } => {
                    self.u8(1);
                    self.str(text);
                    self.u32(*cursor);
                    self.u32(*run_start);
                    self.u32(*run_len);
                }
                Text::ClearMarked => self.u8(2),
            }
        }
    }
    fn nodes(&mut self, nodes: &[Node]) {
        self.u32(nodes.len() as u32);
        for n in nodes {
            self.str(&n.key);
            self.u8(match n.kind {
                NodeKind::Item => 0,
                NodeKind::Toggle => 1,
                NodeKind::Radio => 2,
                NodeKind::Submenu => 3,
            });
            self.str(&n.label);
            self.bool(n.checked);
            self.bool(n.enabled);
            self.nodes(&n.children);
        }
    }
}

pub struct Dec<'a>(&'a [u8]);

fn bad() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, "malformed message")
}

impl<'a> Dec<'a> {
    fn take(&mut self, n: usize) -> io::Result<&'a [u8]> {
        if self.0.len() < n {
            return Err(bad());
        }
        let (a, b) = self.0.split_at(n);
        self.0 = b;
        Ok(a)
    }
    fn u8(&mut self) -> io::Result<u8> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> io::Result<u16> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn u32(&mut self) -> io::Result<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn i32(&mut self) -> io::Result<i32> {
        Ok(i32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn u64(&mut self) -> io::Result<u64> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    fn bool(&mut self) -> io::Result<bool> {
        Ok(self.u8()? != 0)
    }
    fn str(&mut self) -> io::Result<String> {
        let n = self.u32()? as usize;
        String::from_utf8(self.take(n)?.to_vec()).map_err(|_| bad())
    }
    fn opt_str(&mut self) -> io::Result<Option<String>> {
        Ok(if self.bool()? { Some(self.str()?) } else { None })
    }
    fn client(&mut self) -> io::Result<Client> {
        let secure = self.bool()?;
        let left = if self.bool()? { Some((self.str()?, self.bool()?)) } else { None };
        let caret = if self.bool()? { Some([self.i32()?, self.i32()?, self.i32()?, self.i32()?]) } else { None };
        Ok(Client { secure, left, caret })
    }
    fn texts(&mut self) -> io::Result<Vec<Text>> {
        let n = self.u32()?;
        let mut v = Vec::new();
        for _ in 0..n {
            v.push(match self.u8()? {
                0 => Text::Insert(self.str()?),
                1 => Text::Marked { text: self.str()?, cursor: self.u32()?, run_start: self.u32()?, run_len: self.u32()? },
                2 => Text::ClearMarked,
                _ => return Err(bad()),
            });
        }
        Ok(v)
    }
    fn nodes(&mut self, depth: u32) -> io::Result<Vec<Node>> {
        if depth > 4 {
            return Err(bad());
        }
        let n = self.u32()?;
        let mut v = Vec::new();
        for _ in 0..n {
            let key = self.str()?;
            let kind = match self.u8()? {
                0 => NodeKind::Item,
                1 => NodeKind::Toggle,
                2 => NodeKind::Radio,
                3 => NodeKind::Submenu,
                _ => return Err(bad()),
            };
            v.push(Node { key, kind, label: self.str()?, checked: self.bool()?, enabled: self.bool()?, children: self.nodes(depth + 1)? });
        }
        Ok(v)
    }
    fn end(&self) -> io::Result<()> {
        if self.0.is_empty() {
            Ok(())
        } else {
            Err(bad())
        }
    }
}

impl Request {
    pub fn encode(&self) -> Vec<u8> {
        let mut e = Enc::default();
        match self {
            Request::Hello { protocol, app } => {
                e.u8(0);
                e.u32(*protocol);
                e.opt_str(app);
            }
            Request::Listen { session } => {
                e.u8(1);
                e.u64(*session);
            }
            Request::Key { session, key, client } => {
                e.u8(2);
                e.u64(*session);
                e.u16(key.vk);
                e.u16(key.scan);
                e.bool(key.extended);
                e.u8(key.modifiers);
                e.client(client);
            }
            Request::Activate { session, client } => {
                e.u8(3);
                e.u64(*session);
                e.client(client);
            }
            Request::Deactivate { session, client } => {
                e.u8(4);
                e.u64(*session);
                e.client(client);
            }
            Request::Commit { session, client } => {
                e.u8(5);
                e.u64(*session);
                e.client(client);
            }
            Request::Menu { session, client } => {
                e.u8(6);
                e.u64(*session);
                e.client(client);
            }
            Request::Choose { session, key, client } => {
                e.u8(7);
                e.u64(*session);
                e.str(key);
                e.client(client);
            }
            Request::Close { session } => {
                e.u8(8);
                e.u64(*session);
            }
            Request::Modifier { session, key, down } => {
                e.u8(9);
                e.u64(*session);
                e.u16(key.vk);
                e.u16(key.scan);
                e.bool(key.extended);
                e.u8(key.modifiers);
                e.bool(*down);
            }
        }
        e.0
    }

    pub fn decode(b: &[u8]) -> io::Result<Request> {
        let mut d = Dec(b);
        let r = match d.u8()? {
            0 => Request::Hello { protocol: d.u32()?, app: d.opt_str()? },
            1 => Request::Listen { session: d.u64()? },
            2 => {
                let session = d.u64()?;
                let key = RawKey { vk: d.u16()?, scan: d.u16()?, extended: d.bool()?, modifiers: d.u8()? };
                Request::Key { session, key, client: d.client()? }
            }
            3 => Request::Activate { session: d.u64()?, client: d.client()? },
            4 => Request::Deactivate { session: d.u64()?, client: d.client()? },
            5 => Request::Commit { session: d.u64()?, client: d.client()? },
            6 => Request::Menu { session: d.u64()?, client: d.client()? },
            7 => Request::Choose { session: d.u64()?, key: d.str()?, client: d.client()? },
            8 => Request::Close { session: d.u64()? },
            9 => {
                let session = d.u64()?;
                let key = RawKey { vk: d.u16()?, scan: d.u16()?, extended: d.bool()?, modifiers: d.u8()? };
                Request::Modifier { session, key, down: d.bool()? }
            }
            _ => return Err(bad()),
        };
        d.end()?;
        Ok(r)
    }
}

impl Reply {
    pub fn encode(&self) -> Vec<u8> {
        let mut e = Enc::default();
        match self {
            Reply::Session(s) => {
                e.u8(0);
                e.u64(*s);
            }
            Reply::Key { handled, text } => {
                e.u8(1);
                e.bool(*handled);
                e.texts(text);
            }
            Reply::Text(t) => {
                e.u8(2);
                e.texts(t);
            }
            Reply::Menu(n) => {
                e.u8(3);
                e.nodes(n);
            }
            Reply::Done => e.u8(4),
        }
        e.0
    }

    pub fn decode(b: &[u8]) -> io::Result<Reply> {
        let mut d = Dec(b);
        let r = match d.u8()? {
            0 => Reply::Session(d.u64()?),
            1 => Reply::Key { handled: d.bool()?, text: d.texts()? },
            2 => Reply::Text(d.texts()?),
            3 => Reply::Menu(d.nodes(0)?),
            4 => Reply::Done,
            _ => return Err(bad()),
        };
        d.end()?;
        Ok(r)
    }
}

impl Event {
    pub fn encode(&self) -> Vec<u8> {
        let mut e = Enc::default();
        match self {
            Event::Text(t) => {
                e.u8(0);
                e.texts(t);
            }
            Event::MenuChanged => e.u8(1),
        }
        e.0
    }

    pub fn decode(b: &[u8]) -> io::Result<Event> {
        let mut d = Dec(b);
        let r = match d.u8()? {
            0 => Event::Text(d.texts()?),
            1 => Event::MenuChanged,
            _ => return Err(bad()),
        };
        d.end()?;
        Ok(r)
    }
}

/// Messages larger than this are refused (a menu and a page of candidates are a few KB).
pub const MAX_MESSAGE: usize = 1 << 20;

pub fn write_frame(w: &mut impl Write, body: &[u8]) -> io::Result<()> {
    let mut b = Vec::with_capacity(body.len() + 4);
    b.extend_from_slice(&(body.len() as u32).to_le_bytes());
    b.extend_from_slice(body);
    // One write, so a message pipe never sees half a frame.
    w.write_all(&b)?;
    w.flush()
}

pub fn read_frame(r: &mut impl Read) -> io::Result<Vec<u8>> {
    let mut n = [0u8; 4];
    r.read_exact(&mut n)?;
    let n = u32::from_le_bytes(n) as usize;
    if n > MAX_MESSAGE {
        return Err(bad());
    }
    let mut b = vec![0u8; n];
    r.read_exact(&mut b)?;
    Ok(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn client() -> Client {
        Client { secure: true, left: Some(("你好".into(), false)), caret: Some([1, -2, 3, 4]) }
    }

    #[test]
    fn requests_round_trip() {
        let all = [
            Request::Hello { protocol: PROTOCOL, app: Some("discord.exe".into()) },
            Request::Hello { protocol: PROTOCOL, app: None },
            Request::Listen { session: 7 },
            Request::Key { session: 1, key: RawKey { vk: 0x41, scan: 0x1E, extended: false, modifiers: MOD_SHIFT }, client: client() },
            Request::Activate { session: 1, client: Client::default() },
            Request::Deactivate { session: 1, client: client() },
            Request::Commit { session: 1, client: client() },
            Request::Menu { session: 2, client: client() },
            Request::Choose { session: 2, key: "layout.eten".into(), client: client() },
            Request::Close { session: 3 },
            Request::Modifier { session: 3, key: RawKey { vk: 0x10, scan: 0x36, extended: false, modifiers: MOD_SHIFT }, down: false },
        ];
        for r in all {
            assert_eq!(Request::decode(&r.encode()).unwrap(), r);
        }
    }

    #[test]
    fn replies_and_events_round_trip() {
        let texts = vec![
            Text::Insert("你".into()),
            Text::Marked { text: "ㄏ".into(), cursor: 1, run_start: 0, run_len: 0 },
            Text::ClearMarked,
        ];
        let node = Node { key: "clear".into(), kind: NodeKind::Submenu, label: "清除".into(), checked: false, enabled: true, children: vec![
            Node { key: "clear.confirm".into(), kind: NodeKind::Item, label: "清除".into(), checked: false, enabled: true, children: vec![] },
        ] };
        for r in [Reply::Session(9), Reply::Key { handled: true, text: texts.clone() }, Reply::Text(vec![]), Reply::Menu(vec![node]), Reply::Done] {
            assert_eq!(Reply::decode(&r.encode()).unwrap(), r);
        }
        for e in [Event::Text(texts), Event::MenuChanged] {
            assert_eq!(Event::decode(&e.encode()).unwrap(), e);
        }
    }

    #[test]
    fn truncated_or_trailing_bytes_are_refused() {
        let b = Request::Close { session: 3 }.encode();
        assert!(Request::decode(&b[..b.len() - 1]).is_err());
        let mut longer = b.clone();
        longer.push(0);
        assert!(Request::decode(&longer).is_err());
    }

    #[test]
    fn frames() {
        let mut buf = Vec::new();
        write_frame(&mut buf, b"abc").unwrap();
        assert_eq!(read_frame(&mut buf.as_slice()).unwrap(), b"abc");
    }

    #[test]
    fn keys_by_position() {
        let k = |scan, extended| classify(RawKey { vk: 0, scan, extended, modifiers: 0 });
        assert_eq!(k(0x1E, false), Class::Keycap('a'));
        assert_eq!(k(0x1C, true), Class::Special(Special::Enter));
        assert_eq!(k(0x4B, false), Class::Other, "keypad 4 is outside the tables");
        assert_eq!(k(0x4B, true), Class::Special(Special::Left));
        assert_eq!(k(0x49, true), Class::Page { up: true });
        assert_eq!(classify(RawKey { vk: 0x10, scan: 0x2A, extended: false, modifiers: 0 }), Class::Modifier);
        assert_eq!(keycaps().count(), 47);
    }
}
