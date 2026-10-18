//! The host's pipe server: every connection gets a thread; its first message says what it is (`Hello`: a DLL's call
//! pipe, `Listen`: its event pipe). Calls go to the main thread and wait for the reply there.
//!
//! Access: the user (full), AppContainer apps (ALL APPLICATION PACKAGES and ALL RESTRICTED APPLICATION PACKAGES: read
//! and write), at low integrity, and no remote clients: the text service runs inside sandboxed apps too.
use super::{post, wide, Msg};
use shanjie_host::hub::Listener;
use shanjie_proto::{read_frame, write_frame, Event, Request};
use std::fs::File;
use std::os::windows::io::FromRawHandle;
use std::sync::mpsc;
use windows::core::PCWSTR;
use windows::Win32::Foundation::*;
use windows::Win32::Security::Authorization::{ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1};
use windows::Win32::Security::{PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES};
use windows::Win32::Storage::FileSystem::{FILE_FLAG_FIRST_PIPE_INSTANCE, PIPE_ACCESS_DUPLEX};
use windows::Win32::System::Pipes::*;

pub struct EventPipe(File);

impl Listener for EventPipe {
    fn send(&mut self, e: &Event) -> bool {
        write_frame(&mut self.0, &e.encode()).is_ok()
    }
}

pub fn serve(name: String, sid: String) {
    std::thread::spawn(move || unsafe {
        let sddl = format!("D:P(A;;GA;;;{sid})(A;;GRGW;;;AC)(A;;GRGW;;;S-1-15-2-2)S:(ML;;NW;;;LW)");
        let mut descriptor = PSECURITY_DESCRIPTOR::default();
        if ConvertStringSecurityDescriptorToSecurityDescriptorW(PCWSTR(wide(&sddl).as_ptr()), SDDL_REVISION_1, &mut descriptor, None).is_err() {
            return;
        }
        let attributes = SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: descriptor.0,
            bInheritHandle: false.into(),
        };
        let name = wide(&name);
        let mut first = true;
        loop {
            let mode = if first { PIPE_ACCESS_DUPLEX | FILE_FLAG_FIRST_PIPE_INSTANCE } else { PIPE_ACCESS_DUPLEX };
            let pipe = CreateNamedPipeW(
                PCWSTR(name.as_ptr()),
                mode,
                PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
                PIPE_UNLIMITED_INSTANCES,
                64 * 1024,
                64 * 1024,
                0,
                Some(&attributes),
            );
            if pipe == INVALID_HANDLE_VALUE {
                if first {
                    // Another host owns the name.
                    return;
                }
                std::thread::sleep(std::time::Duration::from_millis(200));
                continue;
            }
            first = false;
            let connected = ConnectNamedPipe(pipe, None).is_ok() || GetLastError() == ERROR_PIPE_CONNECTED;
            if !connected {
                let _ = CloseHandle(pipe);
                continue;
            }
            let file = File::from_raw_handle(pipe.0);
            std::thread::spawn(move || connection(file));
        }
    });
}

fn call(r: Request) -> Option<shanjie_proto::Reply> {
    let (tx, rx) = mpsc::channel();
    if !post(Msg::Call(r, tx)) {
        return None;
    }
    rx.recv().ok()
}

fn connection(mut file: File) {
    let Ok(first) = read_frame(&mut file).and_then(|b| Request::decode(&b)) else { return };
    match first {
        Request::Listen { session } => {
            let Ok(writer) = file.try_clone() else { return };
            post(Msg::Listen(session, EventPipe(writer)));
            // The DLL never writes here; a read ends when it closes the pipe.
            let _ = read_frame(&mut file);
        }
        hello @ Request::Hello { .. } => {
            let Some(reply) = call(hello) else { return };
            let session = match reply {
                shanjie_proto::Reply::Session(id) => id,
                _ => 0,
            };
            if write_frame(&mut file, &reply.encode()).is_err() || session == 0 {
                if session != 0 {
                    post(Msg::Gone(session));
                }
                return;
            }
            loop {
                let Ok(r) = read_frame(&mut file).and_then(|b| Request::decode(&b)) else { break };
                // A connection speaks for its own session only.
                if session_of(&r).is_some_and(|s| s != session) {
                    break;
                }
                let closing = matches!(r, Request::Close { .. });
                let Some(reply) = call(r) else { break };
                if write_frame(&mut file, &reply.encode()).is_err() || closing {
                    break;
                }
            }
            post(Msg::Gone(session));
        }
        _ => {}
    }
}

fn session_of(r: &Request) -> Option<u64> {
    match r {
        Request::Hello { .. } => Some(0),
        Request::Listen { session }
        | Request::Key { session, .. }
        | Request::Activate { session, .. }
        | Request::Deactivate { session, .. }
        | Request::Commit { session, .. }
        | Request::Menu { session, .. }
        | Request::Choose { session, .. }
        | Request::Close { session } => Some(*session),
    }
}
