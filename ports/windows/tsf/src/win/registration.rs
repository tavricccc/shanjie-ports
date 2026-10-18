//! The text service's COM class and its one language profile (Traditional Chinese, Taiwan), as the earlier Windows port
//! registered them, with new identifiers so the two never collide. One profile: the layout (標準 / 倚天) is a menu
//! choice, as on macOS, not two keyboards.
use super::wide;
use windows::core::{Result, GUID, PCWSTR};
use windows::Win32::Foundation::*;
use windows::Win32::System::Com::*;
use windows::Win32::System::LibraryLoader::GetModuleFileNameW;
use windows::Win32::System::Registry::*;
use windows::Win32::UI::Input::KeyboardAndMouse::HKL;
use windows::Win32::UI::TextServices::*;

pub const SERVICE: GUID = GUID::from_u128(0x5f0c4a5e_9d3b_4f2e_8c61_2a7b3e9d4c10);
pub const PROFILE: GUID = GUID::from_u128(0x5f0c4a5e_9d3b_4f2e_8c61_2a7b3e9d4c11);
pub const LANGBAR_ITEM: GUID = GUID::from_u128(0x5f0c4a5e_9d3b_4f2e_8c61_2a7b3e9d4c12);
/// The composition's display attributes: the input underline, and the thick one over the English run.
pub const ATTR_INPUT: GUID = GUID::from_u128(0x5f0c4a5e_9d3b_4f2e_8c61_2a7b3e9d4c13);
pub const ATTR_ENGLISH: GUID = GUID::from_u128(0x5f0c4a5e_9d3b_4f2e_8c61_2a7b3e9d4c14);

const LANGID_ZH_TW: u16 = 0x0404;
const NAME: &str = "善解";

const CATEGORIES: [GUID; 5] = [
    GUID_TFCAT_TIP_KEYBOARD,
    GUID_TFCAT_DISPLAYATTRIBUTEPROVIDER,
    GUID_TFCAT_TIPCAP_SYSTRAYSUPPORT,
    GUID_TFCAT_TIPCAP_IMMERSIVESUPPORT,
    GUID_TFCAT_TIPCAP_COMLESS,
];

fn clsid_key() -> String {
    format!("Software\\Classes\\CLSID\\{{{SERVICE:?}}}")
}

unsafe fn set(path: &str, name: Option<&str>, text: &str) -> Result<()> {
    unsafe {
        let mut key = HKEY::default();
        RegCreateKeyExW(HKEY_LOCAL_MACHINE, PCWSTR(wide(path).as_ptr()), Some(0), None, REG_OPTION_NON_VOLATILE, KEY_WRITE, None, &mut key, None).ok()?;
        let w = wide(text);
        let bytes = std::slice::from_raw_parts(w.as_ptr().cast::<u8>(), w.len() * 2);
        let n = name.map(wide);
        let r = RegSetValueExW(key, n.as_ref().map_or(PCWSTR::null(), |n| PCWSTR(n.as_ptr())), Some(0), REG_SZ, Some(bytes));
        let _ = RegCloseKey(key);
        r.ok()
    }
}

pub unsafe fn register(module: HMODULE, remove: bool) -> Result<()> {
    unsafe {
        let initialized = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let result = (|| -> Result<()> {
            let profiles: ITfInputProcessorProfileMgr = CoCreateInstance(&CLSID_TF_InputProcessorProfiles, None, CLSCTX_INPROC_SERVER)?;
            let categories: ITfCategoryMgr = CoCreateInstance(&CLSID_TF_CategoryMgr, None, CLSCTX_INPROC_SERVER)?;
            if remove {
                let r = profiles.UnregisterProfile(&SERVICE, LANGID_ZH_TW, &PROFILE, 0);
                for c in CATEGORIES {
                    let _ = categories.UnregisterCategory(&SERVICE, &c, &SERVICE);
                }
                let _ = RegDeleteTreeW(HKEY_LOCAL_MACHINE, PCWSTR(wide(&clsid_key()).as_ptr()));
                return r;
            }
            let mut buf = [0u16; 32768];
            let n = GetModuleFileNameW(Some(module), &mut buf) as usize;
            if n == 0 || n == buf.len() {
                return Err(windows::core::Error::from_thread());
            }
            let file = String::from_utf16_lossy(&buf[..n]);
            set(&clsid_key(), None, NAME)?;
            set(&format!("{}\\InprocServer32", clsid_key()), None, &file)?;
            set(&format!("{}\\InprocServer32", clsid_key()), Some("ThreadingModel"), "Apartment")?;
            let name: Vec<u16> = NAME.encode_utf16().collect();
            let icon: Vec<u16> = file.encode_utf16().collect();
            profiles.RegisterProfile(&SERVICE, LANGID_ZH_TW, &PROFILE, &name, &icon, 0, HKL::default(), 0, true, 0)?;
            for c in CATEGORIES {
                categories.RegisterCategory(&SERVICE, &c, &SERVICE)?;
            }
            Ok(())
        })();
        if initialized.is_ok() {
            CoUninitialize();
        }
        result
    }
}
