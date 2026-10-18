//! The input method menu (`shanjie_shell::menu_tree`) as IBus properties for the panel's 善 menu: radio items RADIO,
//! switches TOGGLE, submenus MENU, the rest NORMAL (insensitive when there is nothing to choose).
use crate::serial::{self, Property};
use shanjie_shell::menu_tree::{self, Kind, Node};
use shanjie_shell::{Effects, SessionId, Shell};

fn property(n: &Node) -> Property {
    let kind = match n.kind {
        Kind::Item => serial::PROP_TYPE_NORMAL,
        Kind::Toggle => serial::PROP_TYPE_TOGGLE,
        Kind::Radio => serial::PROP_TYPE_RADIO,
        Kind::Submenu => serial::PROP_TYPE_MENU,
    };
    Property::new(&n.key, kind, &n.label, n.checked, n.enabled, n.children.iter().map(property).collect(), "")
}

pub fn build(shell: &Shell, id: SessionId) -> Vec<Property> {
    menu_tree::build(shell, id).iter().map(property).collect()
}

/// PropertyActivate: the panel reports the item's new state.
pub fn activate(shell: &mut Shell, id: SessionId, name: &str, state: u32, out: &mut Effects) {
    menu_tree::activate(shell, id, name, Some(state == serial::PROP_STATE_CHECKED), out);
}
