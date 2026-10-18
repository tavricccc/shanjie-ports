//! The text service object: one per app thread with TSF (ITfTextInputProcessorEx), one host session each.
//!
//! What each TSF callback does (the IMK calls of the macOS input method in brackets):
//! - OnTestKeyDown: claims every key-down except a modifier alone, and except Ctrl / Alt / Windows-key combinations
//!   while nothing is composed here (Ctrl+\ is claimed: the core uses it). OnKeyDown then asks the host in a
//!   synchronous edit session and returns the shell's answer, so a key the shell passes reaches the app [`handle`];
//! - OnSetFocus of the thread manager: the old document's session is deactivated inside an edit session of the context
//!   holding the composition (the shell commits it there), the new one activated [`deactivateServer`/`activateServer`];
//! - OnCompositionTerminated (the app ended the composition, its text already in the document): `Commit`, and the
//!   shell's `Insert` of that same text is not written again [`commitComposition`];
//! - the host's events (a click in the candidate window, a rerank result, another app's key committing the composition
//!   here) are written in an asynchronous edit session on this thread.
//! Before every call the DLL reads, inside the edit session: the input scope (password / PIN fields are the secure
//! input), up to 32 UTF-16 units before the selection (none while composing), and the caret rectangle on screen.
//! UNVERIFIED ON WINDOWS.
use super::client::Client;
use super::registration::{ATTR_ENGLISH, ATTR_INPUT};
use super::{display, langbar, Count};
use shanjie_proto::{classify, Class, Event, Node, RawKey, Reply, Request, Text, MOD_ALT, MOD_CAPSLOCK, MOD_CONTROL, MOD_SHIFT, MOD_WIN};
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::{Rc, Weak};
use std::sync::{Arc, Mutex};
use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::System::Com::*;
use windows::Win32::System::Variant::*;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::TextServices::*;
use windows::Win32::UI::WindowsAndMessaging::*;

const WM_EVENTS: u32 = WM_APP + 7;
/// LeftContext.maxUTF16 (`shanjie_shell::shell::LEFT_CONTEXT_UTF16`).
const LEFT_UTF16: i32 = 32;

struct State {
    thread: Option<ITfThreadMgr>,
    client_id: u32,
    thread_cookie: u32,
    host: Option<Client>,
    /// After a failed connection, no new attempt before this (a key never waits on a host that is not there).
    retry_after: Option<std::time::Instant>,
    sid: Option<String>,
    composition: Option<ITfComposition>,
    focus: Option<ITfContext>,
    atom_input: u32,
    atom_english: u32,
    button: Option<ITfLangBarItemButton>,
    window: HWND,
}

pub struct Inner {
    state: RefCell<State>,
    /// The DLL is ending its own composition: OnCompositionTerminated is not the app's doing.
    ending: Cell<bool>,
    events: Arc<Mutex<VecDeque<Event>>>,
    me: RefCell<Weak<Inner>>,
    /// The COM object, for the sinks TSF needs (borrowed, never owned: the object owns `Inner`).
    owner: Cell<*mut core::ffi::c_void>,
    _count: Count,
}

#[implement(ITfTextInputProcessorEx, ITfKeyEventSink, ITfThreadMgrEventSink, ITfCompositionSink, ITfDisplayAttributeProvider)]
pub struct Service {
    inner: Rc<Inner>,
}

pub fn new() -> ITfTextInputProcessorEx {
    let inner = Rc::new(Inner {
        state: RefCell::new(State {
            thread: None,
            client_id: 0,
            thread_cookie: TF_INVALID_COOKIE,
            host: None,
            retry_after: None,
            sid: None,
            composition: None,
            focus: None,
            atom_input: 0,
            atom_english: 0,
            button: None,
            window: HWND::default(),
        }),
        ending: Cell::new(false),
        events: Arc::new(Mutex::new(VecDeque::new())),
        me: RefCell::new(Weak::new()),
        owner: Cell::new(std::ptr::null_mut()),
        _count: Count::new(),
    });
    *inner.me.borrow_mut() = Rc::downgrade(&inner);
    let object: ITfTextInputProcessorEx = Service { inner: inner.clone() }.into();
    inner.owner.set(object.as_raw());
    object
}

// MARK: edit sessions

#[implement(ITfEditSession)]
struct Edit {
    work: RefCell<Option<Box<dyn FnOnce(u32) -> Result<()>>>>,
}

impl ITfEditSession_Impl for Edit_Impl {
    fn DoEditSession(&self, cookie: u32) -> Result<()> {
        match self.work.borrow_mut().take() {
            Some(f) => f(cookie),
            None => Ok(()),
        }
    }
}

fn utf16(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

fn modifiers() -> u8 {
    let down = |vk: VIRTUAL_KEY| unsafe { GetKeyState(vk.0 as i32) } < 0;
    let mut m = 0;
    if down(VK_SHIFT) {
        m |= MOD_SHIFT;
    }
    if down(VK_CONTROL) {
        m |= MOD_CONTROL;
    }
    if down(VK_MENU) {
        m |= MOD_ALT;
    }
    if down(VK_LWIN) || down(VK_RWIN) {
        m |= MOD_WIN;
    }
    if unsafe { GetKeyState(VK_CAPITAL.0 as i32) } & 1 != 0 {
        m |= MOD_CAPSLOCK;
    }
    m
}

fn raw_key(w: WPARAM, l: LPARAM) -> RawKey {
    RawKey { vk: w.0 as u16, scan: ((l.0 >> 16) & 0xff) as u16, extended: (l.0 >> 24) & 1 != 0, modifiers: modifiers() }
}

unsafe fn selection(ctx: &ITfContext, cookie: u32) -> Option<ITfRange> {
    unsafe {
        let mut sel = [TF_SELECTION::default()];
        let mut n = 0;
        ctx.GetSelection(cookie, TF_DEFAULT_SELECTION, &mut sel, &mut n).ok()?;
        if n == 0 {
            return None;
        }
        std::mem::ManuallyDrop::into_inner(std::mem::take(&mut sel[0].range))
    }
}

unsafe fn set_caret(ctx: &ITfContext, cookie: u32, range: &ITfRange) -> Result<()> {
    unsafe {
        let sel = TF_SELECTION {
            range: std::mem::ManuallyDrop::new(Some(range.clone())),
            style: TF_SELECTIONSTYLE { ase: TF_AE_NONE, fInterimChar: false.into() },
        };
        let mut all = [sel];
        let r = ctx.SetSelection(cookie, &all);
        std::mem::ManuallyDrop::drop(&mut all[0].range);
        r
    }
}

unsafe fn secure(ctx: &ITfContext, cookie: u32, at: &ITfRange) -> bool {
    unsafe {
        let read = || -> Result<bool> {
            let prop = ctx.GetProperty(&GUID_PROP_INPUTSCOPE)?;
            let v = prop.GetValue(cookie, at)?;
            let scope: ITfInputScope = IUnknown::try_from(&v)?.cast()?;
            let mut list: *mut InputScope = std::ptr::null_mut();
            let mut n = 0u32;
            scope.GetInputScopes(&mut list, &mut n)?;
            let found = !list.is_null()
                && std::slice::from_raw_parts(list, n as usize).iter().any(|s| [IS_PASSWORD, IS_NUMERIC_PASSWORD, IS_NUMERIC_PIN, IS_ALPHANUMERIC_PIN].contains(s));
            if !list.is_null() {
                CoTaskMemFree(Some(list.cast()));
            }
            Ok(found)
        };
        read().unwrap_or(false)
    }
}

impl Inner {
    fn owner_unknown(&self) -> Option<IUnknown> {
        unsafe { IUnknown::from_raw_borrowed(&self.owner.get()).cloned() }
    }

    fn request(&self, ctx: &ITfContext, flags: TF_CONTEXT_EDIT_CONTEXT_FLAGS, f: impl FnOnce(u32) -> Result<()> + 'static) -> bool {
        let client = self.state.borrow().client_id;
        let edit: ITfEditSession = Edit { work: RefCell::new(Some(Box::new(f))) }.into();
        unsafe { ctx.RequestEditSession(client, &edit, flags).is_ok_and(|hr| hr.is_ok()) }
    }

    /// The host connection, made on first use (and again after a timeout or a broken pipe).
    fn connected(&self) -> bool {
        if self.state.borrow().host.is_some() {
            return true;
        }
        if self.state.borrow().retry_after.is_some_and(|t| std::time::Instant::now() < t) {
            return false;
        }
        let Some(sid) = self.state.borrow().sid.clone() else { return false };
        let events = self.events.clone();
        let window = self.state.borrow().window.0 as isize;
        let host = Client::connect(&sid, true, move |e| {
            events.lock().unwrap_or_else(|p| p.into_inner()).push_back(e);
            unsafe {
                let _ = PostMessageW(Some(HWND(window as _)), WM_EVENTS, WPARAM(0), LPARAM(0));
            }
        });
        let ok = host.is_some();
        let mut s = self.state.borrow_mut();
        s.host = host;
        s.retry_after = (!ok).then(|| std::time::Instant::now() + std::time::Duration::from_secs(5));
        ok
    }

    /// One call; a host that does not answer is dropped (the next call reconnects).
    fn call(&self, make: impl FnOnce(u64) -> Request) -> Option<Reply> {
        if !self.connected() {
            return None;
        }
        let reply = {
            let s = self.state.borrow();
            let host = s.host.as_ref()?;
            host.call(make(host.session))
        };
        if reply.is_none() {
            self.state.borrow_mut().host = None;
        }
        reply
    }

    fn composing(&self) -> bool {
        self.state.borrow().composition.is_some()
    }

    fn open(&self) -> bool {
        let Some(thread) = self.state.borrow().thread.clone() else { return false };
        unsafe {
            let read = || -> Result<bool> {
                let m: ITfCompartmentMgr = thread.cast()?;
                let v = m.GetCompartment(&GUID_COMPARTMENT_KEYBOARD_OPENCLOSE)?.GetValue()?;
                Ok(i32::try_from(&v).map_or(true, |v| v != 0))
            };
            read().unwrap_or(true)
        }
    }

    /// What the host needs to know about the document, read inside an edit session.
    unsafe fn client(&self, ctx: &ITfContext, cookie: u32) -> shanjie_proto::Client {
        unsafe {
            let comp_range = self.state.borrow().composition.as_ref().and_then(|c| c.GetRange().ok());
            let sel = selection(ctx, cookie);
            let mut c = shanjie_proto::Client::default();
            if let Some(sel) = &sel {
                c.secure = secure(ctx, cookie, sel);
                if comp_range.is_none() {
                    if let Ok(r) = sel.Clone() {
                        let _ = r.Collapse(cookie, TF_ANCHOR_START);
                        let mut moved = 0;
                        if r.ShiftStart(cookie, -LEFT_UTF16, &mut moved, std::ptr::null()).is_ok() {
                            let mut buf = [0u16; 64];
                            let mut n = 0u32;
                            if r.GetText(cookie, 0, &mut buf, &mut n).is_ok() {
                                let text = String::from_utf16_lossy(&buf[..n as usize]);
                                c.left = Some((text, moved == -LEFT_UTF16));
                            }
                        }
                    }
                }
            }
            if let (Ok(view), Some(r)) = (ctx.GetActiveView(), comp_range.or(sel)) {
                let mut rect = RECT::default();
                let mut clipped = BOOL(0);
                if view.GetTextExt(cookie, &r, &mut rect, &mut clipped).is_ok() && rect != RECT::default() {
                    c.caret = Some([rect.left, rect.top, rect.right, rect.bottom]);
                }
            }
            c
        }
    }

    unsafe fn end_composition(&self, cookie: u32) {
        let comp = self.state.borrow_mut().composition.take();
        if let Some(c) = comp {
            self.ending.set(true);
            let _ = unsafe { c.EndComposition(cookie) };
            self.ending.set(false);
        }
    }

    unsafe fn mark(&self, ctx: &ITfContext, cookie: u32, range: &ITfRange, run_start: u32, run_len: u32) {
        unsafe {
            let (input, english) = {
                let s = self.state.borrow();
                (s.atom_input, s.atom_english)
            };
            let Ok(prop) = ctx.GetProperty(&GUID_PROP_ATTRIBUTE) else { return };
            let _ = prop.SetValue(cookie, range, &VARIANT::from(input as i32));
            if run_len > 0 {
                if let Ok(sub) = range.Clone() {
                    let mut moved = 0;
                    let _ = sub.Collapse(cookie, TF_ANCHOR_START);
                    let _ = sub.ShiftEnd(cookie, (run_start + run_len) as i32, &mut moved, std::ptr::null());
                    let _ = sub.ShiftStart(cookie, run_start as i32, &mut moved, std::ptr::null());
                    let _ = prop.SetValue(cookie, &sub, &VARIANT::from(english as i32));
                }
            }
        }
    }

    /// Writes the shell's client calls. `committed`: the app already holds the composition's text (`Insert` skipped).
    unsafe fn apply(&self, ctx: &ITfContext, cookie: u32, texts: Vec<Text>, committed: bool) -> Result<()> {
        unsafe {
            for t in texts {
                match t {
                    Text::Insert(s) => {
                        if committed {
                            continue;
                        }
                        let comp = self.state.borrow().composition.clone();
                        let range = match comp {
                            Some(c) => {
                                let r = c.GetRange()?;
                                r.SetText(cookie, 0, &utf16(&s))?;
                                if let Ok(prop) = ctx.GetProperty(&GUID_PROP_ATTRIBUTE) {
                                    let _ = prop.Clear(cookie, &r);
                                }
                                r
                            }
                            None => ctx.cast::<ITfInsertAtSelection>()?.InsertTextAtSelection(cookie, INSERT_TEXT_AT_SELECTION_FLAGS(0), &utf16(&s))?,
                        };
                        range.Collapse(cookie, TF_ANCHOR_END)?;
                        set_caret(ctx, cookie, &range)?;
                        self.end_composition(cookie);
                    }
                    Text::Marked { text, cursor, run_start, run_len } => {
                        let comp = self.state.borrow().composition.clone();
                        let comp = match comp {
                            Some(c) => c,
                            None => {
                                let at = ctx.cast::<ITfInsertAtSelection>()?.InsertTextAtSelection(cookie, TF_IAS_QUERYONLY, &[])?;
                                let sink: ITfCompositionSink = self.owner_unknown().ok_or(E_FAIL)?.cast()?;
                                let c = ctx.cast::<ITfContextComposition>()?.StartComposition(cookie, &at, &sink)?;
                                self.state.borrow_mut().composition = Some(c.clone());
                                c
                            }
                        };
                        let range = comp.GetRange()?;
                        range.SetText(cookie, 0, &utf16(&text))?;
                        self.mark(ctx, cookie, &range, run_start, run_len);
                        let caret = range.Clone()?;
                        caret.Collapse(cookie, TF_ANCHOR_START)?;
                        let mut moved = 0;
                        caret.ShiftEnd(cookie, cursor as i32, &mut moved, std::ptr::null())?;
                        caret.Collapse(cookie, TF_ANCHOR_END)?;
                        set_caret(ctx, cookie, &caret)?;
                    }
                    Text::ClearMarked => {
                        let comp = self.state.borrow().composition.clone();
                        if let Some(c) = comp {
                            if !committed {
                                let r = c.GetRange()?;
                                r.SetText(cookie, 0, &[])?;
                            }
                            self.end_composition(cookie);
                        }
                    }
                }
            }
            Ok(())
        }
    }

    /// The context the session's text goes to: the composition's, else the focused one.
    fn target(&self) -> Option<ITfContext> {
        let s = self.state.borrow();
        s.composition.as_ref().and_then(|c| unsafe { c.GetRange().ok()?.GetContext().ok() }).or_else(|| s.focus.clone())
    }

    /// A call that may write text, in an asynchronous edit session on `ctx` (or without a document: nothing to write).
    fn edit_call(self: &Rc<Self>, ctx: Option<ITfContext>, make: impl FnOnce(u64, shanjie_proto::Client) -> Request + 'static) {
        let me = self.clone();
        match ctx {
            Some(ctx) => {
                let c2 = ctx.clone();
                self.request(&ctx, TF_ES_ASYNCDONTCARE | TF_ES_READWRITE, move |cookie| unsafe {
                    let client = me.client(&c2, cookie);
                    if let Some(Reply::Text(t)) = me.call(|s| make(s, client)) {
                        me.apply(&c2, cookie, t, false)?;
                    }
                    Ok(())
                });
            }
            None => {
                let _ = self.call(|s| make(s, shanjie_proto::Client::default()));
            }
        }
    }

    fn events(self: &Rc<Self>) {
        let pending: Vec<Event> = self.events.lock().unwrap_or_else(|p| p.into_inner()).drain(..).collect();
        let mut texts = Vec::new();
        for e in pending {
            if let Event::Text(t) = e {
                texts.extend(t);
            }
        }
        if texts.is_empty() {
            return;
        }
        let Some(ctx) = self.target() else { return };
        let me = self.clone();
        let c2 = ctx.clone();
        self.request(&ctx, TF_ES_ASYNCDONTCARE | TF_ES_READWRITE, move |cookie| unsafe { me.apply(&c2, cookie, texts, false) });
    }

    pub fn menu(&self) -> Vec<Node> {
        match self.call(|s| Request::Menu { session: s, client: shanjie_proto::Client::default() }) {
            Some(Reply::Menu(n)) => n,
            _ => Vec::new(),
        }
    }

    pub fn choose(&self, key: String) {
        let Some(me) = self.me.borrow().upgrade() else { return };
        let ctx = me.target();
        me.edit_call(ctx, move |s, client| Request::Choose { session: s, key, client });
    }

    fn focus_changed(self: &Rc<Self>, now: Option<ITfDocumentMgr>) {
        // The composition stays in its own document: the shell commits it there (deactivate) before anything else.
        let old = self.target();
        if old.is_some() || self.composing() {
            self.edit_call(old, |s, client| Request::Deactivate { session: s, client });
        }
        let ctx = now.and_then(|d| unsafe { d.GetTop().ok() });
        self.state.borrow_mut().focus = ctx.clone();
        if let Some(ctx) = ctx {
            self.edit_call(Some(ctx), |s, client| Request::Activate { session: s, client });
        }
    }
}

unsafe extern "system" fn events_proc(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    unsafe {
        if msg == WM_EVENTS {
            let p = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const Weak<Inner>;
            if let Some(inner) = p.as_ref().and_then(|w| w.upgrade()) {
                inner.events();
            }
            return LRESULT(0);
        }
        DefWindowProcW(hwnd, msg, w, l)
    }
}

fn events_window(inner: &Rc<Inner>) -> HWND {
    unsafe {
        let instance = HINSTANCE(super::module().0);
        let class = WNDCLASSW { lpfnWndProc: Some(events_proc), hInstance: instance, lpszClassName: w!("ShanjieTsfEvents"), ..Default::default() };
        RegisterClassW(&class);
        let Ok(hwnd) = CreateWindowExW(WINDOW_EX_STYLE(0), w!("ShanjieTsfEvents"), w!(""), WINDOW_STYLE(0), 0, 0, 0, 0, Some(HWND_MESSAGE), None, Some(instance), None) else {
            return HWND::default();
        };
        let weak = Box::into_raw(Box::new(Rc::downgrade(inner)));
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, weak as isize);
        hwnd
    }
}

fn destroy_events_window(hwnd: HWND) {
    unsafe {
        let p = SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0) as *mut Weak<Inner>;
        if !p.is_null() {
            drop(Box::from_raw(p));
        }
        let _ = DestroyWindow(hwnd);
    }
}

impl ITfTextInputProcessor_Impl for Service_Impl {
    fn Activate(&self, thread: Ref<ITfThreadMgr>, client: u32) -> Result<()> {
        self.ActivateEx(thread, client, 0)
    }

    fn Deactivate(&self) -> Result<()> {
        let inner = &self.inner;
        let ctx = inner.target();
        if inner.composing() {
            inner.edit_call(ctx, |s, client| Request::Deactivate { session: s, client });
        }
        let _ = inner.call(|s| Request::Close { session: s });
        let mut s = inner.state.borrow_mut();
        s.host = None;
        unsafe {
            if let Some(thread) = s.thread.clone() {
                if let Ok(keys) = thread.cast::<ITfKeystrokeMgr>() {
                    let _ = keys.UnadviseKeyEventSink(s.client_id);
                }
                if s.thread_cookie != TF_INVALID_COOKIE {
                    if let Ok(source) = thread.cast::<ITfSource>() {
                        let _ = source.UnadviseSink(s.thread_cookie);
                    }
                }
                if let (Ok(bar), Some(button)) = (thread.cast::<ITfLangBarItemMgr>(), s.button.take()) {
                    let _ = bar.RemoveItem(&button);
                }
            }
        }
        s.thread_cookie = TF_INVALID_COOKIE;
        s.thread = None;
        s.focus = None;
        s.composition = None;
        let window = std::mem::take(&mut s.window);
        drop(s);
        if !window.is_invalid() {
            destroy_events_window(window);
        }
        Ok(())
    }
}

impl ITfTextInputProcessorEx_Impl for Service_Impl {
    fn ActivateEx(&self, thread: Ref<ITfThreadMgr>, client: u32, _flags: u32) -> Result<()> {
        let inner = &self.inner;
        let thread = thread.ok()?.clone();
        let owner = inner.owner_unknown().ok_or(E_FAIL)?;
        unsafe {
            let categories: ITfCategoryMgr = CoCreateInstance(&CLSID_TF_CategoryMgr, None, CLSCTX_INPROC_SERVER)?;
            let atom_input = categories.RegisterGUID(&ATTR_INPUT).unwrap_or(0);
            let atom_english = categories.RegisterGUID(&ATTR_ENGLISH).unwrap_or(0);
            let source: ITfSource = thread.cast()?;
            let cookie = source.AdviseSink(&ITfThreadMgrEventSink::IID, &owner)?;
            let sink: ITfKeyEventSink = owner.cast()?;
            thread.cast::<ITfKeystrokeMgr>()?.AdviseKeyEventSink(client, &sink, true)?;
            let button = langbar::new(Rc::downgrade(inner));
            if let Ok(bar) = thread.cast::<ITfLangBarItemMgr>() {
                let _ = bar.AddItem(&button);
            }
            // An IME is open when it is chosen: a 中/英 switch is not upstream's (macOS switches input sources instead).
            if let Ok(m) = thread.cast::<ITfCompartmentMgr>() {
                if let Ok(c) = m.GetCompartment(&GUID_COMPARTMENT_KEYBOARD_OPENCLOSE) {
                    let _ = c.SetValue(client, &VARIANT::from(1i32));
                }
            }
            let window = events_window(inner);
            {
                let mut s = inner.state.borrow_mut();
                s.thread = Some(thread.clone());
                s.client_id = client;
                s.thread_cookie = cookie;
                s.atom_input = atom_input;
                s.atom_english = atom_english;
                s.button = Some(button);
                s.window = window;
                s.sid = sid();
            }
            inner.focus_changed(thread.GetFocus().ok());
        }
        Ok(())
    }
}

fn sid() -> Option<String> {
    use windows::Win32::Security::Authorization::ConvertSidToStringSidW;
    use windows::Win32::Security::{GetTokenInformation, TokenUser, TOKEN_QUERY, TOKEN_USER};
    use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
    unsafe {
        let mut token = HANDLE::default();
        OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).ok()?;
        let mut len = 0u32;
        let _ = GetTokenInformation(token, TokenUser, None, 0, &mut len);
        let mut buf = vec![0u8; len as usize];
        let got = GetTokenInformation(token, TokenUser, Some(buf.as_mut_ptr().cast()), len, &mut len);
        let _ = CloseHandle(token);
        got.ok()?;
        let user = &*(buf.as_ptr() as *const TOKEN_USER);
        let mut text = PWSTR::null();
        ConvertSidToStringSidW(user.User.Sid, &mut text).ok()?;
        let s = text.to_string().ok();
        let _ = LocalFree(Some(HLOCAL(text.0.cast())));
        s
    }
}

impl ITfThreadMgrEventSink_Impl for Service_Impl {
    fn OnInitDocumentMgr(&self, _: Ref<ITfDocumentMgr>) -> Result<()> {
        Ok(())
    }
    fn OnUninitDocumentMgr(&self, _: Ref<ITfDocumentMgr>) -> Result<()> {
        Ok(())
    }
    fn OnSetFocus(&self, now: Ref<ITfDocumentMgr>, _prev: Ref<ITfDocumentMgr>) -> Result<()> {
        self.inner.focus_changed(now.cloned());
        Ok(())
    }
    fn OnPushContext(&self, _: Ref<ITfContext>) -> Result<()> {
        Ok(())
    }
    fn OnPopContext(&self, _: Ref<ITfContext>) -> Result<()> {
        Ok(())
    }
}

impl ITfKeyEventSink_Impl for Service_Impl {
    fn OnSetFocus(&self, _foreground: BOOL) -> Result<()> {
        Ok(())
    }

    fn OnTestKeyDown(&self, _ctx: Ref<ITfContext>, w: WPARAM, l: LPARAM) -> Result<BOOL> {
        let inner = &self.inner;
        if !inner.open() {
            return Ok(false.into());
        }
        let k = raw_key(w, l);
        let class = classify(k);
        if class == Class::Modifier {
            return Ok(false.into());
        }
        let chord = k.modifiers & (MOD_CONTROL | MOD_ALT | MOD_WIN) != 0;
        let ctrl_backslash = k.modifiers & (MOD_CONTROL | MOD_ALT | MOD_WIN) == MOD_CONTROL && class == Class::Keycap('\\');
        let claim = inner.composing() || !chord || ctrl_backslash;
        Ok((claim && inner.connected()).into())
    }

    fn OnKeyDown(&self, ctx: Ref<ITfContext>, w: WPARAM, l: LPARAM) -> Result<BOOL> {
        let inner = self.inner.clone();
        let Some(ctx) = ctx.cloned() else { return Ok(false.into()) };
        if !inner.open() {
            return Ok(false.into());
        }
        let k = raw_key(w, l);
        if classify(k) == Class::Modifier {
            return Ok(false.into());
        }
        let handled = Rc::new(Cell::new(false));
        let h = handled.clone();
        let c2 = ctx.clone();
        let me = inner.clone();
        let ran = inner.request(&ctx, TF_ES_SYNC | TF_ES_READWRITE, move |cookie| unsafe {
            let client = me.client(&c2, cookie);
            if let Some(Reply::Key { handled, text }) = me.call(|s| Request::Key { session: s, key: k, client }) {
                h.set(handled);
                me.apply(&c2, cookie, text, false)?;
            }
            Ok(())
        });
        Ok((ran && handled.get()).into())
    }

    fn OnTestKeyUp(&self, _: Ref<ITfContext>, _: WPARAM, _: LPARAM) -> Result<BOOL> {
        Ok(false.into())
    }
    fn OnKeyUp(&self, _: Ref<ITfContext>, _: WPARAM, _: LPARAM) -> Result<BOOL> {
        Ok(false.into())
    }
    fn OnPreservedKey(&self, _: Ref<ITfContext>, _: *const GUID) -> Result<BOOL> {
        Ok(false.into())
    }
}

impl ITfCompositionSink_Impl for Service_Impl {
    fn OnCompositionTerminated(&self, cookie: u32, comp: Ref<ITfComposition>) -> Result<()> {
        let inner = &self.inner;
        if inner.ending.get() {
            return Ok(());
        }
        let ctx = comp.as_ref().and_then(|c| unsafe { c.GetRange().ok()?.GetContext().ok() });
        let client = match &ctx {
            Some(c) => unsafe { inner.client(c, cookie) },
            None => shanjie_proto::Client::default(),
        };
        if let Some(Reply::Text(t)) = inner.call(|s| Request::Commit { session: s, client }) {
            if let Some(ctx) = &ctx {
                unsafe {
                    let _ = inner.apply(ctx, cookie, t, true);
                }
            }
        }
        inner.state.borrow_mut().composition = None;
        Ok(())
    }
}

impl ITfDisplayAttributeProvider_Impl for Service_Impl {
    fn EnumDisplayAttributeInfo(&self) -> Result<IEnumTfDisplayAttributeInfo> {
        Ok(display::all())
    }
    fn GetDisplayAttributeInfo(&self, guid: *const GUID) -> Result<ITfDisplayAttributeInfo> {
        unsafe { display::info(&*guid) }
    }
}
