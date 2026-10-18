//! The host's logic, free of Win32 so it builds and is tested on any platform with the real core and data
//! (`tests/hub.rs`); `main.rs` and its Windows modules wire it to pipes, a message loop and the candidate window.
pub mod hub;
pub mod keys;
pub mod paths;
pub mod tables;
