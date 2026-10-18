//! COM entry points and the module's object count.
mod client;
mod display;
mod icon;
mod langbar;
mod registration;
mod service;

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::OnceLock;
use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::System::Com::*;
use windows::Win32::System::LibraryLoader::{GetModuleHandleExW, GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS, GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT};

static MODULE: OnceLock<isize> = OnceLock::new();
static OBJECTS: AtomicU32 = AtomicU32::new(0);

/// Counts a live COM object of this DLL (DllCanUnloadNow).
pub struct Count;

impl Count {
    pub fn new() -> Self {
        OBJECTS.fetch_add(1, Ordering::Relaxed);
        Count
    }
}

impl Drop for Count {
    fn drop(&mut self) {
        OBJECTS.fetch_sub(1, Ordering::Relaxed);
    }
}

pub fn module() -> HMODULE {
    HMODULE(*MODULE.get_or_init(|| unsafe {
        let mut m = HMODULE::default();
        let _ = GetModuleHandleExW(
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
            PCWSTR(DllGetClassObject as *const u16),
            &mut m,
        );
        m.0 as isize
    }) as _)
}

pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

#[implement(IClassFactory)]
struct Factory {
    _count: Count,
}

impl IClassFactory_Impl for Factory_Impl {
    fn CreateInstance(&self, outer: Ref<IUnknown>, iid: *const GUID, out: *mut *mut core::ffi::c_void) -> Result<()> {
        unsafe {
            *out = std::ptr::null_mut();
            if outer.is_some() {
                return Err(CLASS_E_NOAGGREGATION.into());
            }
            service::new().query(iid, out).ok()
        }
    }

    fn LockServer(&self, lock: BOOL) -> Result<()> {
        if lock.as_bool() {
            OBJECTS.fetch_add(1, Ordering::Relaxed);
        } else {
            OBJECTS.fetch_sub(1, Ordering::Relaxed);
        }
        Ok(())
    }
}

#[no_mangle]
pub extern "system" fn DllCanUnloadNow() -> HRESULT {
    if OBJECTS.load(Ordering::Relaxed) == 0 {
        S_OK
    } else {
        S_FALSE
    }
}

#[no_mangle]
pub unsafe extern "system" fn DllGetClassObject(id: *const GUID, iid: *const GUID, out: *mut *mut core::ffi::c_void) -> HRESULT {
    unsafe {
        if *id != registration::SERVICE {
            *out = std::ptr::null_mut();
            return CLASS_E_CLASSNOTAVAILABLE;
        }
        let f: IClassFactory = Factory { _count: Count::new() }.into();
        f.query(iid, out)
    }
}

#[no_mangle]
pub unsafe extern "system" fn DllRegisterServer() -> HRESULT {
    match unsafe { registration::register(module(), false) } {
        Ok(()) => S_OK,
        Err(e) => {
            let _ = unsafe { registration::register(module(), true) };
            e.code()
        }
    }
}

#[no_mangle]
pub unsafe extern "system" fn DllUnregisterServer() -> HRESULT {
    match unsafe { registration::register(module(), true) } {
        Ok(()) => S_OK,
        Err(e) => e.code(),
    }
}
