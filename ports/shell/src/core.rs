//! Port of macos/Sources/ShanjieKit/Core.swift: one `ShanjieEngine` handle, every call through the C ABI
//! (core/include/shanjie.h), every output copied into owned values and freed in the call that received it.
//!
//! R2 (docs/contracts/s3b.md section 9): the types holding input text have no `Debug`, and nothing here logs.
use shanjie_core::ffi::{self, ShanjieEngine, ShanjieKey, ShanjieOutput, ShanjieRerankJob};
use std::ffi::{c_char, CStr, CString};
use std::ptr;

/// `SHANJIE_PACK_ACG` (shanjie.h).
pub const PACK_ACG: u32 = 1;
/// `SHANJIE_NO_CHANGE` (shanjie.h): `shanjie_engine_rerank_apply` changed nothing.
pub const NO_CHANGE: i32 = 5;

/// One output of the core (Core.swift `CoreOutput`).
#[derive(Clone, Default, PartialEq)]
pub struct CoreOutput {
    pub handled: bool,
    pub commit: String,
    pub preedit: String,
    pub cursor_utf16: u32,
    pub candidates: Vec<String>,
    pub selected: i32,
    /// s3b2 section 8.2: 0 = collapsed single row, > 0 = expanded grid with this many columns.
    pub columns: u32,
    pub first: u32,
    pub total: u32,
    /// candidate-vertical contract section 2.2: 0 horizontal (or none), 1 vertical candidate window, 2 vertical prediction row.
    pub vertical: u32,
    /// zhen-inline section 1.3f: the open English run in `preedit`, UTF-16 start and length.
    pub english_run_start: u32,
    pub english_run_len: u32,
}

/// The result of a core call: an output, or the non-zero C ABI return code (Core.swift `CoreResult`).
pub enum CoreResult {
    Ok(CoreOutput),
    Failed(i32),
}

/// Owns one engine handle. Not `Send`: the handle is used on one thread only (s3a section 6), as the Swift shell keeps it
/// on the main thread.
pub struct CoreEngine {
    handle: *mut ShanjieEngine,
}

fn cstring(s: &str) -> Option<CString> {
    CString::new(s).ok()
}

unsafe fn string(p: *const c_char) -> String {
    if p.is_null() {
        String::new()
    } else {
        unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned()
    }
}

/// Copies every field, then frees the C output at once (Core.swift `take`).
fn take(code: i32, out: *mut ShanjieOutput) -> CoreResult {
    if code != 0 || out.is_null() {
        // SAFETY: NULL on a non-zero code (s3a section 6); freeing NULL does nothing.
        unsafe { ffi::shanjie_output_free(out) };
        return CoreResult::Failed(if code == 0 { 4 } else { code });
    }
    // SAFETY: a live output from the core, read once and freed below.
    let o = unsafe { &*out };
    let mut candidates = Vec::with_capacity(o.candidate_count as usize);
    if !o.candidates.is_null() {
        for i in 0..o.candidate_count as usize {
            // SAFETY: `candidates` holds `candidate_count` pointers.
            let c = unsafe { *o.candidates.add(i) };
            if !c.is_null() {
                candidates.push(unsafe { string(c) });
            }
        }
    }
    let result = CoreOutput {
        handled: o.handled != 0,
        commit: unsafe { string(o.commit) },
        preedit: unsafe { string(o.preedit) },
        cursor_utf16: o.cursor_utf16,
        candidates,
        selected: o.candidate_selected,
        columns: o.candidate_columns,
        first: o.candidate_first,
        total: o.candidate_total,
        vertical: o.candidate_vertical,
        english_run_start: o.english_run_utf16_start,
        english_run_len: o.english_run_utf16_len,
    };
    // SAFETY: returned by the core and not freed yet.
    unsafe { ffi::shanjie_output_free(out) };
    CoreResult::Ok(result)
}

macro_rules! snapshot {
    ($self:ident, $f:ident $(, $arg:expr)*) => {{
        let mut out: *mut ShanjieOutput = ptr::null_mut();
        // SAFETY: a live handle and a valid out pointer.
        let code = unsafe { ffi::$f($self.handle $(, $arg)*, &mut out) };
        take(code, out)
    }};
}

impl CoreEngine {
    /// `shanjie_engine_new`, or `shanjie_engine_new_packs` with the ACG pack when `packs_dir` is given.
    pub fn make(data_dir: &str, layout: u32, packs_dir: Option<&str>) -> Result<CoreEngine, i32> {
        let dir = cstring(data_dir).ok_or(2)?;
        let mut out: *mut ShanjieEngine = ptr::null_mut();
        let code = match packs_dir {
            Some(p) => {
                let packs = cstring(p).ok_or(2)?;
                // SAFETY: valid C strings and out pointer.
                unsafe { ffi::shanjie_engine_new_packs(dir.as_ptr(), layout, packs.as_ptr(), PACK_ACG, &mut out) }
            }
            // SAFETY: as above.
            None => unsafe { ffi::shanjie_engine_new(dir.as_ptr(), layout, &mut out) },
        };
        if code != 0 || out.is_null() {
            return Err(if code == 0 { 4 } else { code });
        }
        Ok(CoreEngine { handle: out })
    }

    fn path_call(&self, path: &str, f: unsafe extern "C" fn(*mut ShanjieEngine, *const c_char) -> i32) -> i32 {
        match cstring(path) {
            // SAFETY: a live handle and a valid C string.
            Some(p) => unsafe { f(self.handle, p.as_ptr()) },
            None => 2,
        }
    }

    pub fn load_lm(&self, path: &str) -> i32 {
        self.path_call(path, ffi::shanjie_engine_load_lm)
    }

    pub fn load_tinylm(&self, path: &str) -> i32 {
        self.path_call(path, ffi::shanjie_engine_load_tinylm)
    }

    pub fn load_english(&self, path: &str) -> i32 {
        self.path_call(path, ffi::shanjie_engine_load_english)
    }

    pub fn learning_open(&self, dir: &str) -> i32 {
        self.path_call(dir, ffi::shanjie_engine_learning_open)
    }

    pub fn set_punctuation(&self, table: &str) -> i32 {
        self.path_call(table, ffi::shanjie_engine_set_punctuation)
    }

    /// `None` is no left context (NULL).
    pub fn set_left_context(&self, text: Option<&str>) -> i32 {
        match text {
            Some(t) => self.path_call(t, ffi::shanjie_engine_set_left_context),
            // SAFETY: a live handle; NULL is allowed.
            None => unsafe { ffi::shanjie_engine_set_left_context(self.handle, ptr::null()) },
        }
    }

    pub fn set_learning(&self, enabled: bool) -> i32 {
        // SAFETY: a live handle.
        unsafe { ffi::shanjie_engine_set_learning(self.handle, enabled as u32) }
    }

    pub fn learning_clear(&self) -> i32 {
        // SAFETY: a live handle.
        unsafe { ffi::shanjie_engine_learning_clear(self.handle) }
    }

    /// bit0: a full rewrite of the learning file failed (S4 section 4); `None` on a non-zero code.
    pub fn learning_status(&self) -> Option<u32> {
        let mut flags = 0u32;
        // SAFETY: a live handle and a valid out pointer.
        (unsafe { ffi::shanjie_engine_learning_status(self.handle, &mut flags) } == 0).then_some(flags)
    }

    /// The levels the loaded small model offers (bit0 conservative, bit1 standard, bit2 aggressive); 0 without one.
    pub fn rerank_levels(&self) -> u32 {
        let mut mask = 0u32;
        // SAFETY: a live handle and a valid out pointer.
        let code = unsafe { ffi::shanjie_engine_rerank_levels(self.handle, &mut mask) };
        if code == 0 {
            mask
        } else {
            0
        }
    }

    pub fn set_rerank(&self, on: bool) -> CoreResult {
        snapshot!(self, shanjie_engine_set_rerank, on as u32)
    }

    pub fn set_rerank_level(&self, level: u32) -> CoreResult {
        snapshot!(self, shanjie_engine_set_rerank_level, level)
    }

    pub fn set_demote(&self, on: bool) -> CoreResult {
        snapshot!(self, shanjie_engine_set_demote, on as u32)
    }

    pub fn set_prediction(&self, on: bool) -> CoreResult {
        snapshot!(self, shanjie_engine_set_prediction, on as u32)
    }

    pub fn set_candidate_vertical(&self, on: bool) -> CoreResult {
        snapshot!(self, shanjie_engine_set_candidate_vertical, on as u32)
    }

    pub fn set_abbreviation(&self, on: bool) -> CoreResult {
        snapshot!(self, shanjie_engine_set_abbreviation, on as u32)
    }

    pub fn set_english(&self, on: bool) -> CoreResult {
        snapshot!(self, shanjie_engine_set_english, on as u32)
    }

    pub fn set_english_inline(&self, on: bool) -> CoreResult {
        snapshot!(self, shanjie_engine_set_english_inline, on as u32)
    }

    pub fn set_profile(&self, profile: u32) -> CoreResult {
        snapshot!(self, shanjie_engine_set_profile, profile)
    }

    pub fn key(&self, key: ShanjieKey) -> CoreResult {
        snapshot!(self, shanjie_engine_key, key)
    }

    pub fn pick(&self, index: u32) -> CoreResult {
        snapshot!(self, shanjie_engine_pick, index)
    }

    /// mode 0 commits the composition then clears, 1 discards.
    pub fn reset(&self, mode: u32) -> CoreResult {
        snapshot!(self, shanjie_engine_reset, mode)
    }

    /// The job of the last change, once: `Ok(None)` when there is none.
    pub fn rerank_take(&self) -> Result<Option<RerankJob>, i32> {
        let mut job: *mut ShanjieRerankJob = ptr::null_mut();
        // SAFETY: a live handle and a valid out pointer.
        let code = unsafe { ffi::shanjie_engine_rerank_take(self.handle, &mut job) };
        if code != 0 {
            return Err(code);
        }
        Ok((!job.is_null()).then_some(RerankJob { job }))
    }

    /// `shanjie_engine_rerank_apply`: `Ok` with the snapshot when the composition changed, `Failed(5)` for no change.
    pub fn rerank_apply(&self, job: &RerankJob) -> CoreResult {
        let mut out: *mut ShanjieOutput = ptr::null_mut();
        // SAFETY: a live handle, a job that has run (the shell's runner guarantees the order), a valid out pointer.
        let code = unsafe { ffi::shanjie_engine_rerank_apply(self.handle, job.job, &mut out) };
        take(code, out)
    }
}

impl Drop for CoreEngine {
    fn drop(&mut self) {
        // SAFETY: the live handle, freed once.
        unsafe { ffi::shanjie_engine_free(self.handle) };
    }
}

/// One rerank job (Core.swift `RerankJob`). `Send`: the header allows `shanjie_rerank_run` on another thread and the free on
/// any thread once run returned; the shell runs one job at a time and applies it only after run returned (the same three
/// invariants the Swift wrapper documents).
pub struct RerankJob {
    job: *mut ShanjieRerankJob,
}

// SAFETY: see the type's comment; the job owns its data and holds no engine borrow (ffi.rs `ShanjieRerankJob`).
unsafe impl Send for RerankJob {}

impl RerankJob {
    /// `shanjie_rerank_run`, off the main thread: 0, or 4.
    pub fn run(&self) -> i32 {
        // SAFETY: a live job, run by one thread at a time.
        unsafe { ffi::shanjie_rerank_run(self.job) }
    }
}

impl Drop for RerankJob {
    fn drop(&mut self) {
        // SAFETY: the live job, freed once.
        unsafe { ffi::shanjie_rerank_free(self.job) };
    }
}
