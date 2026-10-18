//! Windows stand-ins for the few POSIX calls the core makes (ports/docs/core-windows.md). Nothing here changes what the
//! core computes: learning records, model scores and decoding are the same code on every platform. Only the operating
//! system calls underneath differ.
//!
//! - `learn_store.rs` keeps its Unix code; on Windows it imports the extension traits below under the same names. The
//!   POSIX owner and mode checks have no direct Windows equivalent: the learning directory sits in the user's own
//!   profile (`%LOCALAPPDATA%`), which Windows already restricts to that user, so `uid` always matches `geteuid` and the
//!   modes read back as the ones the store wrote (0700 for directories, 0600 for files). `O_NOFOLLOW` opens a reparse
//!   point (a symlink or a junction) itself rather than its target, so the store's `is_file` check refuses it as on Unix.
//! - `trigram.rs` maps its table with `CreateFileMappingW` / `MapViewOfFile` instead of `mmap`.
//! - `learn.rs` reads the local calendar day with `GetLocalTime` instead of `localtime_r`.
#![cfg(windows)]

use std::ffi::c_void;
use std::fs::{DirBuilder, Metadata, OpenOptions, Permissions};

/// `O_NOFOLLOW`, `O_NONBLOCK` and `ELOOP` as learn_store.rs names them. Only `O_NOFOLLOW` means anything here (a FIFO
/// cannot be created in the directory on Windows); `ELOOP` is never returned, so its branch is never taken.
pub const O_NOFOLLOW: i32 = 0x0100;
pub const O_NONBLOCK: i32 = 0x0004;
pub const ELOOP: i32 = -1;

/// `FILE_FLAG_OPEN_REPARSE_POINT` (winbase.h).
const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;

/// The same user always: see the module comment.
///
/// # Safety
/// None; `unsafe` only so the Unix call sites compile unchanged.
pub unsafe fn geteuid() -> u32 {
    0
}

pub trait DirBuilderExt {
    fn mode(&mut self, mode: u32) -> &mut Self;
}

impl DirBuilderExt for DirBuilder {
    fn mode(&mut self, _mode: u32) -> &mut Self {
        self
    }
}

pub trait OpenOptionsExt {
    fn mode(&mut self, mode: u32) -> &mut Self;
    fn custom_flags(&mut self, flags: i32) -> &mut Self;
}

impl OpenOptionsExt for OpenOptions {
    fn mode(&mut self, _mode: u32) -> &mut Self {
        self
    }

    fn custom_flags(&mut self, flags: i32) -> &mut Self {
        if flags & O_NOFOLLOW != 0 {
            std::os::windows::fs::OpenOptionsExt::custom_flags(self, FILE_FLAG_OPEN_REPARSE_POINT);
        }
        self
    }
}

pub trait MetadataExt {
    fn uid(&self) -> u32;
    fn mode(&self) -> u32;
    fn nlink(&self) -> u64;
}

impl MetadataExt for Metadata {
    fn uid(&self) -> u32 {
        0
    }

    fn mode(&self) -> u32 {
        if self.is_dir() {
            0o700
        } else {
            0o600
        }
    }

    /// Hard links of the learning file are not something Windows users make; std has no stable reader for the count.
    fn nlink(&self) -> u64 {
        1
    }
}

pub trait PermissionsExt {
    fn from_mode(mode: u32) -> Self;
}

impl PermissionsExt for Permissions {
    /// Windows permissions carry only the read-only attribute: every mode the store asks for is writable by its owner, so
    /// this is "not read-only". std has no constructor, so the value is taken from the temporary directory's metadata.
    fn from_mode(_mode: u32) -> Self {
        let mut p = std::fs::metadata(std::env::temp_dir()).map(|m| m.permissions()).unwrap_or_else(|_| {
            std::fs::metadata(".").expect("a readable current directory").permissions()
        });
        #[allow(clippy::permissions_set_readonly_false)]
        p.set_readonly(false);
        p
    }
}

#[repr(C)]
struct SystemTime {
    year: u16,
    month: u16,
    day_of_week: u16,
    day: u16,
    hour: u16,
    minute: u16,
    second: u16,
    milliseconds: u16,
}

#[link(name = "kernel32")]
extern "system" {
    fn GetLocalTime(out: *mut SystemTime);
    fn CreateFileMappingW(file: *mut c_void, attributes: *mut c_void, protect: u32, high: u32, low: u32, name: *const u16) -> *mut c_void;
    fn MapViewOfFile(mapping: *mut c_void, access: u32, high: u32, low: u32, len: usize) -> *mut c_void;
    fn UnmapViewOfFile(base: *const c_void) -> i32;
    fn CloseHandle(handle: *mut c_void) -> i32;
}

/// Days since 1970-01-01 of a proleptic Gregorian date (Howard Hinnant's `days_from_civil`).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The local calendar day as days since 1970-01-01: the same value `learn::local_day` computes from `localtime_r` on Unix.
pub fn local_day() -> i64 {
    let mut t = SystemTime { year: 1970, month: 1, day_of_week: 0, day: 1, hour: 0, minute: 0, second: 0, milliseconds: 0 };
    // SAFETY: GetLocalTime fills the structure and cannot fail.
    unsafe { GetLocalTime(&mut t) };
    days_from_civil(t.year as i64, t.month as i64, t.day as i64)
}

/// A whole file mapped read-only, as `mmap(PROT_READ, MAP_PRIVATE)` does on Unix. Windows keeps a mapped file from being
/// replaced or deleted until it is unmapped; the installers replace the data folder by version instead (ports/windows).
pub fn map_read_only(file: &std::fs::File, len: usize) -> Option<*mut c_void> {
    use std::os::windows::io::AsRawHandle;
    const PAGE_READONLY: u32 = 0x02;
    const FILE_MAP_READ: u32 = 0x0004;
    // SAFETY: a valid open handle; a zero size maps the whole file; the mapping handle is closed once the view exists
    // (the view keeps the mapping alive).
    unsafe {
        let mapping = CreateFileMappingW(file.as_raw_handle(), std::ptr::null_mut(), PAGE_READONLY, 0, 0, std::ptr::null());
        if mapping.is_null() {
            return None;
        }
        let view = MapViewOfFile(mapping, FILE_MAP_READ, 0, 0, len);
        CloseHandle(mapping);
        (!view.is_null()).then_some(view)
    }
}

/// Unmaps a view `map_read_only` returned.
///
/// # Safety
/// `view` must be a pointer `map_read_only` returned and not yet unmapped.
pub unsafe fn unmap(view: *mut c_void) {
    UnmapViewOfFile(view);
}

#[cfg(test)]
mod tests {
    #[test]
    fn civil_days() {
        assert_eq!(super::days_from_civil(1970, 1, 1), 0);
        assert_eq!(super::days_from_civil(2000, 3, 1), 11_017);
        assert_eq!(super::days_from_civil(9999, 12, 31), 2_932_896);
    }
}
