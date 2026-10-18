//! The language bar / tray button (善) and its menu: `shanjie_shell::menu_tree` as the host sends it, drawn with ITfMenu
//! (toggles with a checkmark, radio items with a dot, greyed when disabled, submenus). The menu is fetched every time it
//! opens, so it always shows the current settings.
use super::registration::{LANGBAR_ITEM, SERVICE};
use super::service::Inner;
use super::Count;
use shanjie_proto::{Node, NodeKind};
use std::cell::RefCell;
use std::rc::Weak;
use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::UI::TextServices::*;
use windows::Win32::UI::WindowsAndMessaging::HICON;

#[implement(ITfLangBarItemButton, ITfSource)]
pub struct Button {
    owner: Weak<Inner>,
    sink: RefCell<Option<ITfLangBarItemSink>>,
    /// Menu id - 1 -> node key, for the menu last opened.
    keys: RefCell<Vec<String>>,
    _count: Count,
}

pub fn new(owner: Weak<Inner>) -> ITfLangBarItemButton {
    Button { owner, sink: RefCell::new(None), keys: RefCell::new(Vec::new()), _count: Count::new() }.into()
}

fn add(menu: &ITfMenu, nodes: &[Node], keys: &mut Vec<String>) -> Result<()> {
    for n in nodes {
        keys.push(n.key.clone());
        let id = keys.len() as u32;
        let mut flags = 0;
        match n.kind {
            NodeKind::Toggle if n.checked => flags |= TF_LBMENUF_CHECKED,
            NodeKind::Radio if n.checked => flags |= TF_LBMENUF_RADIOCHECKED,
            NodeKind::Submenu => flags |= TF_LBMENUF_SUBMENU,
            _ => {}
        }
        if !n.enabled {
            flags |= TF_LBMENUF_GRAYED;
        }
        let label: Vec<u16> = n.label.encode_utf16().collect();
        let mut sub: Option<ITfMenu> = None;
        unsafe {
            menu.AddMenuItem(
                id,
                flags,
                Default::default(),
                Default::default(),
                &label,
                if n.kind == NodeKind::Submenu { &mut sub } else { std::ptr::null_mut() },
            )?;
        }
        if let Some(sub) = sub {
            add(&sub, &n.children, keys)?;
        }
    }
    Ok(())
}

impl ITfLangBarItem_Impl for Button_Impl {
    fn GetInfo(&self, out: *mut TF_LANGBARITEMINFO) -> Result<()> {
        unsafe {
            *out = TF_LANGBARITEMINFO {
                clsidService: SERVICE,
                guidItem: LANGBAR_ITEM,
                dwStyle: TF_LBI_STYLE_BTN_MENU | TF_LBI_STYLE_SHOWNINTRAY,
                ..Default::default()
            };
            let text: Vec<u16> = "善解".encode_utf16().collect();
            (&mut (*out).szDescription)[..text.len()].copy_from_slice(&text);
        }
        Ok(())
    }
    fn GetStatus(&self) -> Result<u32> {
        Ok(0)
    }
    fn Show(&self, _show: BOOL) -> Result<()> {
        Ok(())
    }
    fn GetTooltipString(&self) -> Result<BSTR> {
        Ok(BSTR::from("善解"))
    }
}

impl ITfLangBarItemButton_Impl for Button_Impl {
    fn OnClick(&self, _click: TfLBIClick, _pt: &POINT, _area: *const RECT) -> Result<()> {
        Ok(())
    }
    fn InitMenu(&self, menu: Ref<ITfMenu>) -> Result<()> {
        let menu = menu.ok()?;
        let nodes = self.owner.upgrade().map(|o| o.menu()).unwrap_or_default();
        let mut keys = Vec::new();
        add(menu, &nodes, &mut keys)?;
        *self.keys.borrow_mut() = keys;
        Ok(())
    }
    fn OnMenuSelect(&self, id: u32) -> Result<()> {
        let key = self.keys.borrow().get((id as usize).wrapping_sub(1)).cloned();
        if let (Some(key), Some(owner)) = (key, self.owner.upgrade()) {
            owner.choose(key);
        }
        Ok(())
    }
    fn GetIcon(&self) -> Result<HICON> {
        unsafe { super::icon::make() }
    }
    fn GetText(&self) -> Result<BSTR> {
        Ok(BSTR::from("善"))
    }
}

impl ITfSource_Impl for Button_Impl {
    fn AdviseSink(&self, iid: *const GUID, sink: Ref<IUnknown>) -> Result<u32> {
        unsafe {
            if *iid != ITfLangBarItemSink::IID {
                return Err(E_NOINTERFACE.into());
            }
        }
        *self.sink.borrow_mut() = Some(sink.ok()?.cast()?);
        Ok(1)
    }
    fn UnadviseSink(&self, cookie: u32) -> Result<()> {
        if cookie != 1 {
            return Err(E_INVALIDARG.into());
        }
        self.sink.borrow_mut().take();
        Ok(())
    }
}
