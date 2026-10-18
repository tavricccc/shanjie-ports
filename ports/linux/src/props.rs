//! The input method menu as IBus properties (the panel's 善 menu). The entries are `Shell::menu` in its order, with its
//! titles, checkmarks and greying:
//! - 標準鍵盤 / 倚天鍵盤 are RADIO, the switches TOGGLE, the lines without an action insensitive NORMAL items;
//! - 善解設定… opens a window on macOS. IBus has no settings window of its own, so the window's settings that the menu does
//!   not already hold appear where 善解設定… stands, one submenu each (a panel shows one level of submenus): 切換方式,
//!   個別設定的 App (activating an app removes it, the window's 移除), 英文的顯示, 強度, 候選窗方向. The glass tint slider
//!   has nothing to tint here (the desktop draws the candidate window) and is left out.
//! - 清除選字記憶… asks first on macOS (an alert with 清除 and 取消). Here it is a submenu with those two items, the alert's
//!   title as its first line.
use crate::serial::{self, Property};
use shanjie_shell::menu::{dialog, settings_text as st, text, Action, InputMode};
use shanjie_shell::shell::{ProfileMode, RerankLevel};
use shanjie_shell::{Effects, SessionId, Shell};

const CHECKED: u32 = serial::PROP_STATE_CHECKED;

fn info(key: &str, label: &str) -> Property {
    Property::new(key, serial::PROP_TYPE_NORMAL, label, false, false, Vec::new(), "")
}

fn item(key: &str, label: &str, sensitive: bool) -> Property {
    Property::new(key, serial::PROP_TYPE_NORMAL, label, false, sensitive, Vec::new(), "")
}

fn radio(key: &str, label: &str, checked: bool, sensitive: bool) -> Property {
    Property::new(key, serial::PROP_TYPE_RADIO, label, checked, sensitive, Vec::new(), "")
}

fn menu(key: &str, label: &str, sensitive: bool, sub: Vec<Property>) -> Property {
    Property::new(key, serial::PROP_TYPE_MENU, label, false, sensitive, sub, "")
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

fn settings_menus(shell: &Shell) -> Vec<Property> {
    let s = shell.settings();
    let mut out = Vec::new();
    out.push(menu(
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
                item(&format!("set.apps.remove:{app}"), &format!("{app}（{kind}）— {}", st::REMOVE), true)
            })
            .collect()
    };
    out.push(menu("set.apps", st::APP_PROFILES, true, apps));
    out.push(menu(
        "set.englishInline",
        text::ENGLISH_DISPLAY,
        s.english_inline_enabled,
        vec![
            radio("set.englishInline.typed", text::ENGLISH_DISPLAY_TYPED, s.english_inline, s.english_inline_enabled),
            radio("set.englishInline.space", text::ENGLISH_DISPLAY_SPACE, !s.english_inline, s.english_inline_enabled),
        ],
    ));
    let e = s.rerank_level_enabled;
    out.push(menu(
        "set.rerankLevel",
        &format!("{}{}", text::RERANK, st::RERANK_LEVEL),
        e,
        vec![
            radio("set.rerankLevel.conservative", st::RERANK_CONSERVATIVE, s.rerank_level == RerankLevel::Conservative, e),
            radio("set.rerankLevel.standard", st::RERANK_STANDARD, s.rerank_level == RerankLevel::Standard, e),
            radio("set.rerankLevel.aggressive", st::RERANK_AGGRESSIVE, s.rerank_level == RerankLevel::Aggressive, e),
        ],
    ));
    out.push(menu(
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

pub fn build(shell: &Shell, id: SessionId) -> Vec<Property> {
    let mut props = Vec::new();
    for (n, e) in shell.menu(id).into_iter().enumerate() {
        match e.action {
            None => props.push(info(&format!("info.{n}"), &e.title)),
            Some(a @ Action::Layout(_)) => props.push(radio(action_key(a), &e.title, e.checked, e.enabled)),
            Some(Action::OpenSettings) => props.extend(settings_menus(shell)),
            Some(Action::Clear) => props.push(menu(
                "clear",
                &e.title,
                e.enabled,
                vec![
                    info("clear.title", dialog::CLEAR_TITLE),
                    item("clear.confirm", dialog::CLEAR_BUTTON, true),
                    item("clear.cancel", dialog::CANCEL, true),
                ],
            )),
            Some(a) => props.push(Property::new(action_key(a), serial::PROP_TYPE_TOGGLE, &e.title, e.checked, e.enabled, Vec::new(), "")),
        }
    }
    props
}

/// PropertyActivate. A RADIO item is acted on when it becomes checked; a TOGGLE when its new state differs from the
/// setting (the panel flips its own checkmark, the shell then re-registers the menu either way).
pub fn activate(shell: &mut Shell, id: SessionId, name: &str, state: u32, out: &mut Effects) {
    let checked = state == CHECKED;
    let current = |shell: &Shell, a: Action| shell.menu(id).into_iter().find(|e| e.action == Some(a)).map(|e| e.checked);
    match name {
        "layout.standard" if checked => shell.perform(id, Action::Layout(InputMode::Standard), out),
        "layout.eten" if checked => shell.perform(id, Action::Layout(InputMode::Eten), out),
        "clear.confirm" => shell.clear_learning(id, out),
        "set.profileMode.auto" if checked => shell.set_profile_mode(ProfileMode::Auto, out),
        "set.profileMode.chat" if checked => shell.set_profile_mode(ProfileMode::Chat, out),
        "set.profileMode.formal" if checked => shell.set_profile_mode(ProfileMode::Formal, out),
        "set.englishInline.typed" if checked => shell.apply_english_inline(true, out),
        "set.englishInline.space" if checked => shell.apply_english_inline(false, out),
        "set.rerankLevel.conservative" if checked => shell.apply_rerank_level(RerankLevel::Conservative, out),
        "set.rerankLevel.standard" if checked => shell.apply_rerank_level(RerankLevel::Standard, out),
        "set.rerankLevel.aggressive" if checked => shell.apply_rerank_level(RerankLevel::Aggressive, out),
        "set.orientation.horizontal" if checked => shell.apply_candidate_vertical(false, out),
        "set.orientation.vertical" if checked => shell.apply_candidate_vertical(true, out),
        _ => {
            if let Some(app) = name.strip_prefix("set.apps.remove:") {
                shell.remove_app_profile(app, out);
                return;
            }
            if let Some(a) = TOGGLES.into_iter().find(|a| action_key(*a) == name) {
                if current(shell, a).is_some_and(|c| c != checked) {
                    shell.perform(id, a, out);
                }
            }
        }
    }
}
