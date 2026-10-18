//! The composition's look (ITfDisplayAttributeProvider): the input underline everywhere, and a thick one over the
//! English run (macOS: the thick underline the shell asks for).
use super::registration::{ATTR_ENGLISH, ATTR_INPUT};
use super::Count;
use std::cell::Cell;
use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::UI::TextServices::*;

#[implement(ITfDisplayAttributeInfo)]
pub struct Info {
    guid: GUID,
    _count: Count,
}

pub fn info(guid: &GUID) -> Result<ITfDisplayAttributeInfo> {
    if *guid != ATTR_INPUT && *guid != ATTR_ENGLISH {
        return Err(E_INVALIDARG.into());
    }
    Ok(Info { guid: *guid, _count: Count::new() }.into())
}

impl ITfDisplayAttributeInfo_Impl for Info_Impl {
    fn GetGUID(&self) -> Result<GUID> {
        Ok(self.guid)
    }
    fn GetDescription(&self) -> Result<BSTR> {
        Ok(BSTR::from(if self.guid == ATTR_ENGLISH { "善解 英文" } else { "善解 輸入" }))
    }
    fn GetAttributeInfo(&self, out: *mut TF_DISPLAYATTRIBUTE) -> Result<()> {
        unsafe {
            *out = TF_DISPLAYATTRIBUTE {
                crText: TF_DA_COLOR { r#type: TF_CT_NONE, ..Default::default() },
                crBk: TF_DA_COLOR { r#type: TF_CT_NONE, ..Default::default() },
                lsStyle: TF_LS_SOLID,
                fBoldLine: (self.guid == ATTR_ENGLISH).into(),
                crLine: TF_DA_COLOR { r#type: TF_CT_NONE, ..Default::default() },
                bAttr: TF_ATTR_INPUT,
            };
        }
        Ok(())
    }
    fn SetAttributeInfo(&self, _: *const TF_DISPLAYATTRIBUTE) -> Result<()> {
        Err(E_NOTIMPL.into())
    }
    fn Reset(&self) -> Result<()> {
        Ok(())
    }
}

#[implement(IEnumTfDisplayAttributeInfo)]
pub struct Enum {
    at: Cell<usize>,
    _count: Count,
}

pub fn all() -> IEnumTfDisplayAttributeInfo {
    Enum { at: Cell::new(0), _count: Count::new() }.into()
}

const ALL: [GUID; 2] = [ATTR_INPUT, ATTR_ENGLISH];

impl IEnumTfDisplayAttributeInfo_Impl for Enum_Impl {
    fn Clone(&self) -> Result<IEnumTfDisplayAttributeInfo> {
        Ok(Enum { at: Cell::new(self.at.get()), _count: Count::new() }.into())
    }
    fn Next(&self, count: u32, out: *mut Option<ITfDisplayAttributeInfo>, fetched: *mut u32) -> Result<()> {
        let mut n = 0u32;
        while n < count && self.at.get() < ALL.len() {
            unsafe { *out.add(n as usize) = Some(info(&ALL[self.at.get()])?) };
            self.at.set(self.at.get() + 1);
            n += 1;
        }
        if !fetched.is_null() {
            unsafe { *fetched = n };
        }
        if n == count { Ok(()) } else { Err(S_FALSE.into()) }
    }
    fn Reset(&self) -> Result<()> {
        self.at.set(0);
        Ok(())
    }
    fn Skip(&self, count: u32) -> Result<()> {
        self.at.set((self.at.get() + count as usize).min(ALL.len()));
        Ok(())
    }
}
