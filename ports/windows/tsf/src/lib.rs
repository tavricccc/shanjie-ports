//! shanjie_tsf.dll: the text service Windows loads into every app (TSF), the IMK client side of the macOS input method.
//! It decides nothing: keys, focus changes and menu choices go to shanjie-host.exe (`shanjie_proto`), which owns the
//! shell; the DLL reads the document for it (left context, password field, caret) and writes the composition it answers
//! with. Builds on Windows only. UNVERIFIED ON WINDOWS: cross-compiled only (ports/windows/README.md).
#![cfg(windows)]
mod win;
pub use win::*;
