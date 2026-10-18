//! The host process on Windows: one instance per user, the shell on the main thread (its engine is used on one thread
//! only, as the macOS input method's main thread does), a message loop, the pipe server on its own threads, the rerank
//! worker. UNVERIFIED ON WINDOWS.
mod candidate;
mod pipe;
mod sid;

use shanjie_host::hub::{Dialogs, Hub};
use shanjie_proto::{Reply, Request};
use shanjie_shell::core::RerankJob;
use shanjie_shell::menu::dialog;
use shanjie_shell::prefs::{Defaults, Prefs};
use shanjie_shell::{RerankRunner, Setup, Shell};
use std::collections::HashMap;
use std::sync::{mpsc, OnceLock};
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::HiDpi::{SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2};
use windows::Win32::UI::WindowsAndMessaging::*;

pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

/// What the other threads hand to the main thread.
pub enum Msg {
    Call(Request, mpsc::Sender<Reply>),
    Listen(u64, pipe::EventPipe),
    /// A DLL's call pipe broke: the app quit or crashed.
    Gone(u64),
    Click(u32),
    Rerank(u64, RerankJob, i32),
}

const WM_WAKE: u32 = WM_APP + 1;

struct Wake {
    tx: mpsc::Sender<Msg>,
    hwnd: isize,
}

static WAKE: OnceLock<Wake> = OnceLock::new();

/// Queues `m` for the main thread and wakes its message loop.
pub fn post(m: Msg) -> bool {
    let Some(w) = WAKE.get() else { return false };
    if w.tx.send(m).is_err() {
        return false;
    }
    unsafe { PostMessageW(Some(HWND(w.hwnd as _)), WM_WAKE, WPARAM(0), LPARAM(0)).is_ok() }
}

struct Runner(mpsc::Sender<(u64, RerankJob)>);

impl RerankRunner for Runner {
    fn run(&self, ticket: u64, job: RerankJob) {
        let _ = self.0.send((ticket, job));
    }
}

struct MessageBoxes;

impl Dialogs for MessageBoxes {
    /// Learning.swift `clearFailed`: its title and message; the button is the system's OK (upstream: 好).
    fn clear_failed(&mut self) {
        std::thread::spawn(|| unsafe {
            MessageBoxW(
                None,
                PCWSTR(wide(dialog::FAILED_MESSAGE).as_ptr()),
                PCWSTR(wide(dialog::FAILED_TITLE).as_ptr()),
                MB_OK | MB_ICONWARNING | MB_TOPMOST | MB_SETFOREGROUND,
            );
        });
    }
}

unsafe extern "system" fn wake_proc(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    unsafe { DefWindowProcW(hwnd, msg, w, l) }
}

pub fn run() {
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }
    let Some(sid) = sid::current_user() else { return };
    // One host per user: a second start leaves at once.
    let mutex = unsafe { CreateMutexW(None, false, PCWSTR(wide(&shanjie_proto::mutex_name(&sid)).as_ptr())) };
    if mutex.is_err() || unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        return;
    }

    let instance: HINSTANCE = unsafe { GetModuleHandleW(None) }.map(Into::into).unwrap_or_default();
    let class = WNDCLASSW { lpfnWndProc: Some(wake_proc), hInstance: instance, lpszClassName: w!("ShanjieHostWake"), ..Default::default() };
    unsafe { RegisterClassW(&class) };
    let Ok(wake) = (unsafe {
        CreateWindowExW(WINDOW_EX_STYLE(0), w!("ShanjieHostWake"), w!(""), WINDOW_STYLE(0), 0, 0, 0, 0, Some(HWND_MESSAGE), None, Some(instance), None)
    }) else {
        return;
    };
    let (tx, rx) = mpsc::channel::<Msg>();
    let _ = WAKE.set(Wake { tx, hwnd: wake.0 as isize });

    let data = shanjie_host::paths::data();
    let (defaults, _problems) = Defaults::load(&data.join("defaults.ini"));
    let prefs = shanjie_host::paths::settings().map(Prefs::open).unwrap_or_else(Prefs::memory);
    let setup = Setup {
        resources: data,
        learning_directory: shanjie_host::paths::learning(),
        tables: shanjie_host::tables::tables(),
        shows_abbreviation: false,
        punctuation: None,
        names: HashMap::new(),
    };
    let mut shell = Shell::new(setup, prefs, &defaults);
    let (jobs_tx, jobs_rx) = mpsc::channel::<(u64, RerankJob)>();
    std::thread::spawn(move || {
        for (ticket, job) in jobs_rx {
            let code = job.run();
            if !post(Msg::Rerank(ticket, job, code)) {
                break;
            }
        }
    });
    shell.set_rerank_runner(Box::new(Runner(jobs_tx)));
    let panel = candidate::Window::create(instance);
    let mut hub = Hub::new(shell, Box::new(panel), Box::new(MessageBoxes));

    pipe::serve(shanjie_proto::pipe_name(&sid), sid);

    let mut m = MSG::default();
    while unsafe { GetMessageW(&mut m, None, 0, 0) }.as_bool() {
        if m.message == WM_WAKE {
            while let Ok(msg) = rx.try_recv() {
                match msg {
                    Msg::Call(r, reply) => {
                        let _ = reply.send(hub.request(r));
                    }
                    Msg::Listen(session, events) => {
                        hub.listen(session, Box::new(events));
                    }
                    Msg::Gone(session) => hub.close(session),
                    Msg::Click(index) => hub.clicked(index),
                    Msg::Rerank(ticket, job, code) => hub.rerank_finished(ticket, job, code),
                }
            }
            continue;
        }
        unsafe {
            let _ = TranslateMessage(&m);
            DispatchMessageW(&m);
        }
    }
    let _ = mutex;
}
