//! The DLL's connection to shanjie-host.exe: the call pipe (each call waits at most `TIMEOUT`, on a worker thread, so a
//! stuck host never freezes the app: the key then passes and the connection is dropped) and the event pipe (a reader
//! thread hands every event to `on_event`, which wakes the app's thread).
use super::wide;
use shanjie_proto::{pipe_name, read_frame, write_frame, Event, Reply, Request, PROTOCOL};
use std::fs::File;
use std::os::windows::io::FromRawHandle;
use std::sync::mpsc;
use std::time::{Duration, Instant};
use windows::core::{PCWSTR, PWSTR};
use windows::Win32::Foundation::*;
use windows::Win32::Storage::FileSystem::*;
use windows::Win32::System::LibraryLoader::GetModuleFileNameW;
use windows::Win32::System::Pipes::WaitNamedPipeW;
use windows::Win32::System::Threading::*;

const TIMEOUT: Duration = Duration::from_millis(800);

pub struct Client {
    tx: mpsc::Sender<(Request, mpsc::Sender<Option<Reply>>)>,
    pub session: u64,
}

fn open(name: &[u16]) -> Option<File> {
    unsafe {
        for _ in 0..3 {
            match CreateFileW(PCWSTR(name.as_ptr()), (GENERIC_READ | GENERIC_WRITE).0, FILE_SHARE_NONE, None, OPEN_EXISTING, FILE_FLAGS_AND_ATTRIBUTES(0), None) {
                Ok(h) => return Some(File::from_raw_handle(h.0)),
                Err(e) if e.code() == ERROR_PIPE_BUSY.to_hresult() => {
                    let _ = WaitNamedPipeW(PCWSTR(name.as_ptr()), 300);
                }
                Err(_) => return None,
            }
        }
        None
    }
}

/// shanjie-host.exe beside this DLL. Fails quietly where processes cannot be started (AppContainer apps): the host is
/// started at sign-in by the installer anyway. Whether the process started.
fn launch_host() -> bool {
    unsafe {
        let mut buf = [0u16; 32768];
        let n = GetModuleFileNameW(Some(super::module()), &mut buf) as usize;
        let dll = std::path::PathBuf::from(String::from_utf16_lossy(&buf[..n]));
        let Some(dir) = dll.parent() else { return false };
        let exe = dir.join("shanjie-host.exe");
        let mut cmd = wide(&format!("\"{}\"", exe.display()));
        let si = STARTUPINFOW { cb: std::mem::size_of::<STARTUPINFOW>() as u32, ..Default::default() };
        let mut pi = PROCESS_INFORMATION::default();
        let app = wide(&exe.to_string_lossy());
        if CreateProcessW(PCWSTR(app.as_ptr()), Some(PWSTR(cmd.as_mut_ptr())), None, None, false, DETACHED_PROCESS, None, None, &si, &mut pi).is_ok() {
            let _ = CloseHandle(pi.hThread);
            let _ = CloseHandle(pi.hProcess);
            return true;
        }
        false
    }
}

/// The app's identity: its executable's file name, lower case (`discord.exe`).
pub fn app_id() -> Option<String> {
    unsafe {
        let mut buf = [0u16; 32768];
        let n = GetModuleFileNameW(None, &mut buf) as usize;
        let path = std::path::PathBuf::from(String::from_utf16_lossy(&buf[..n]));
        path.file_name().map(|f| f.to_string_lossy().to_lowercase())
    }
}

impl Client {
    /// `on_event` runs on the reader thread.
    pub fn connect(sid: &str, may_launch: bool, on_event: impl Fn(Event) + Send + 'static) -> Option<Client> {
        let name = wide(&pipe_name(sid));
        let mut call = match open(&name) {
            Some(f) => f,
            None if may_launch => {
                if !launch_host() {
                    return None;
                }
                let start = Instant::now();
                loop {
                    std::thread::sleep(Duration::from_millis(100));
                    if let Some(f) = open(&name) {
                        break f;
                    }
                    if start.elapsed() > Duration::from_secs(3) {
                        return None;
                    }
                }
            }
            None => return None,
        };
        write_frame(&mut call, &Request::Hello { protocol: PROTOCOL, app: app_id() }.encode()).ok()?;
        let session = match Reply::decode(&read_frame(&mut call).ok()?).ok()? {
            Reply::Session(id) if id != 0 => id,
            _ => return None,
        };
        let mut events = open(&name)?;
        write_frame(&mut events, &Request::Listen { session }.encode()).ok()?;
        std::thread::spawn(move || {
            while let Ok(b) = read_frame(&mut events) {
                if let Ok(e) = Event::decode(&b) {
                    on_event(e);
                }
            }
        });
        let (tx, rx) = mpsc::channel::<(Request, mpsc::Sender<Option<Reply>>)>();
        std::thread::spawn(move || {
            for (r, reply) in rx {
                let got = write_frame(&mut call, &r.encode()).ok().and_then(|_| read_frame(&mut call).ok()).and_then(|b| Reply::decode(&b).ok());
                let broken = got.is_none();
                let _ = reply.send(got);
                if broken {
                    break;
                }
            }
        });
        Some(Client { tx, session })
    }

    /// `None`: the host did not answer in time or the pipe broke; the caller drops the client.
    pub fn call(&self, r: Request) -> Option<Reply> {
        let (tx, rx) = mpsc::channel();
        self.tx.send((r, tx)).ok()?;
        rx.recv_timeout(TIMEOUT).ok().flatten()
    }
}
