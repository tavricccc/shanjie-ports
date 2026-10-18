//! The input method menu as one tree both ports draw with their own menus (IBus properties, the TSF language bar menu).
//! The entries are `Shell::menu` in its order, with its titles, checkmarks and greying:
//! - 標準鍵盤 / 倚天鍵盤 are radio items, the switches toggles, the lines without an action insensitive items;
//! - 善解設定… opens a window on macOS. The ports have no settings window, so the window's settings that the menu does
//!   not already hold appear where 善解設定… stands, one submenu each (a panel shows one level of submenus): 切換方式,
//!   個別設定的 App (choosing an app removes it, the window's 移除), 英文的顯示, then 中英文之間自動加空格 as a toggle
//!   (its explanatory line has no place in a menu), 語意校正強度, 候選窗方向. The glass tint slider has nothing to tint
//!   on the ports and is left out.
//! - 清除選字記憶… asks first on macOS (an alert with 清除 and 取消). Here it is a submenu with those two items, the alert's
//!   title as its first line.
//! A node is chosen by its key (`activate`).
use crate::menu::{dialog, settings_text as st, text, Action, InputMode};
use crate::shell::{ProfileMode, RerankLevel};
use crate::{Effects, SessionId, Shell};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A line to read or an item to choose.
    Item,
    Toggle,
    Radio,
    Submenu,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    pub key: String,
    pub kind: Kind,
    pub label: String,
    pub checked: bool,
    pub enabled: bool,
    pub children: Vec<Node>,
}

fn node(key: &str, kind: Kind, label: &str, checked: bool, enabled: bool, children: Vec<Node>) -> Node {
    Node { key: key.into(), kind, label: label.into(), checked, enabled, children }
}

fn info(key: &str, label: &str) -> Node {
    node(key, Kind::Item, label, false, false, Vec::new())
}

fn item(key: &str, label: &str) -> Node {
    node(key, Kind::Item, label, false, true, Vec::new())
}

fn radio(key: &str, label: &str, checked: bool, enabled: bool) -> Node {
    node(key, Kind::Radio, label, checked, enabled, Vec::new())
}

fn submenu(key: &str, label: &str, enabled: bool, children: Vec<Node>) -> Node {
    node(key, Kind::Submenu, label, false, enabled, children)
}

fn action_key(a: Action) -> &'static str {
    match a {
        Action::Layout(InputMode::Standard) => "layout.standard",
        Action::Layout(InputMode::Eten) => "layout.eten",
        Action::Clear => "clear",
        Action::ToggleDemote => "demote",
        Action::TogglePrediction => "prediction",
        Action::ToggleEnglish => "english",
        Action::ToggleAbbreviation => "abbreviation",
        Action::ToggleRerank => "rerank",
        Action::ToggleAcgPack => "acgPack",
        Action::ToggleAppChat => "appChat",
        Action::OpenSettings => "settings",
    }
}

const TOGGLES: [Action; 7] = [
    Action::ToggleDemote,
    Action::TogglePrediction,
    Action::ToggleEnglish,
    Action::ToggleAbbreviation,
    Action::ToggleRerank,
    Action::ToggleAcgPack,
    Action::ToggleAppChat,
];

const REMOVE_APP: &str = "set.apps.remove:";

fn settings_submenus(shell: &Shell) -> Vec<Node> {
    let s = shell.settings();
    let mut out = Vec::new();
    out.push(submenu(
        "set.profileMode",
        st::PROFILE_MODE,
        true,
        vec![
            radio("set.profileMode.auto", st::PROFILE_AUTO, s.profile_mode == ProfileMode::Auto, true),
            radio("set.profileMode.chat", st::PROFILE_CHAT, s.profile_mode == ProfileMode::Chat, true),
            radio("set.profileMode.formal", st::PROFILE_FORMAL, s.profile_mode == ProfileMode::Formal, true),
            info("set.profileMode.note", st::PROFILE_NOTE),
        ],
    ));
    let apps = if s.app_profiles.is_empty() {
        vec![info("set.apps.empty", &st::app_profiles_empty())]
    } else {
        s.app_profiles
            .iter()
            .map(|(app, chat)| {
                let kind = if *chat { st::CHAT } else { st::FORMAL };
                item(&format!("{REMOVE_APP}{app}"), &format!("{app}（{kind}）— {}", st::REMOVE))
            })
            .collect()
    };
    out.push(submenu("set.apps", st::APP_PROFILES, true, apps));
    let e = s.english_inline_enabled;
    out.push(submenu(
        "set.englishInline",
        text::ENGLISH_DISPLAY,
        e,
        vec![
            radio("set.englishInline.typed", text::ENGLISH_DISPLAY_TYPED, s.english_inline, e),
            radio("set.englishInline.space", text::ENGLISH_DISPLAY_SPACE, !s.english_inline, e),
        ],
    ));
    // Always enabled, independent of the 中英混打 switch (cjk-spacing section 2).
    out.push(node("set.spacing", Kind::Toggle, text::SPACING, s.spacing, true, Vec::new()));
    let e = s.rerank_level_enabled;
    out.push(submenu(
        "set.rerankLevel",
        &format!("{}{}", text::RERANK, st::RERANK_LEVEL),
        e,
        vec![
            radio("set.rerankLevel.conservative", st::RERANK_CONSERVATIVE, s.rerank_level == RerankLevel::Conservative, e),
            radio("set.rerankLevel.standard", st::RERANK_STANDARD, s.rerank_level == RerankLevel::Standard, e),
            radio("set.rerankLevel.aggressive", st::RERANK_AGGRESSIVE, s.rerank_level == RerankLevel::Aggressive, e),
        ],
    ));
    out.push(submenu(
        "set.orientation",
        st::ORIENTATION,
        true,
        vec![
            radio("set.orientation.horizontal", st::HORIZONTAL, !s.candidate_vertical, true),
            radio("set.orientation.vertical", st::VERTICAL, s.candidate_vertical, true),
        ],
    ));
    out
}

/// The menu for session `id`.
pub fn build(shell: &Shell, id: SessionId) -> Vec<Node> {
    let mut nodes = Vec::new();
    for (n, e) in shell.menu(id).into_iter().enumerate() {
        match e.action {
            None => nodes.push(info(&format!("info.{n}"), &e.title)),
            Some(a @ Action::Layout(_)) => nodes.push(radio(action_key(a), &e.title, e.checked, e.enabled)),
            Some(Action::OpenSettings) => nodes.extend(settings_submenus(shell)),
            Some(Action::Clear) => nodes.push(submenu(
                "clear",
                &e.title,
                e.enabled,
                vec![info("clear.title", dialog::CLEAR_TITLE), item("clear.confirm", dialog::CLEAR_BUTTON), item("clear.cancel", dialog::CANCEL)],
            )),
            Some(a) => nodes.push(node(action_key(a), Kind::Toggle, &e.title, e.checked, e.enabled, Vec::new())),
        }
    }
    nodes
}

/// Choosing the node `key`. `checked`: the state the menu reports after the choice, for menus that flip their own
/// checkmark (IBus); `None` for menus that only report the choice (TSF), where a toggle flips and a radio item selects.
/// A radio item acts only when it becomes checked; a toggle only when the new state differs from the setting.
pub fn activate(shell: &mut Shell, id: SessionId, key: &str, checked: Option<bool>, out: &mut Effects) {
    let selects = checked.unwrap_or(true);
    match key {
        "layout.standard" if selects => shell.perform(id, Action::Layout(InputMode::Standard), out),
        "layout.eten" if selects => shell.perform(id, Action::Layout(InputMode::Eten), out),
        "clear.confirm" => shell.clear_learning(id, out),
        "set.profileMode.auto" if selects => shell.set_profile_mode(ProfileMode::Auto, out),
        "set.profileMode.chat" if selects => shell.set_profile_mode(ProfileMode::Chat, out),
        "set.profileMode.formal" if selects => shell.set_profile_mode(ProfileMode::Formal, out),
        "set.englishInline.typed" if selects => shell.apply_english_inline(true, out),
        "set.englishInline.space" if selects => shell.apply_english_inline(false, out),
        "set.rerankLevel.conservative" if selects => shell.apply_rerank_level(RerankLevel::Conservative, out),
        "set.rerankLevel.standard" if selects => shell.apply_rerank_level(RerankLevel::Standard, out),
        "set.rerankLevel.aggressive" if selects => shell.apply_rerank_level(RerankLevel::Aggressive, out),
        "set.orientation.horizontal" if selects => shell.apply_candidate_vertical(false, out),
        "set.orientation.vertical" if selects => shell.apply_candidate_vertical(true, out),
        "set.spacing" => {
            let on = checked.unwrap_or(!shell.settings().spacing);
            if on != shell.settings().spacing {
                shell.set_spacing(on, out);
            }
        }
        _ => {
            if let Some(app) = key.strip_prefix(REMOVE_APP) {
                shell.remove_app_profile(app, out);
                return;
            }
            let Some(a) = TOGGLES.into_iter().find(|a| action_key(*a) == key) else { return };
            let current = shell.menu(id).into_iter().find(|e| e.action == Some(a)).map(|e| (e.checked, e.enabled));
            match (current, checked) {
                (Some((_, false)), _) | (None, _) => {}
                (Some((now, true)), Some(want)) if now == want => {}
                _ => shell.perform(id, a, out),
            }
        }
    }
}
