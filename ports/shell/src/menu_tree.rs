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
//! - The Windows habits (winops.rs), which upstream does not have, follow the settings as two more submenus: Windows 操作
//!   (everything but the punctuation, in groups under a line each) and Windows 操作：標點 (one switch per mark).
//! A node is chosen by its key (`activate`).
use crate::menu::{dialog, settings_text as st, text, Action, InputMode};
use crate::shell::{ProfileMode, RerankLevel};
use crate::winops::{self, key as wk, ShiftSide};
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

fn toggle(key: &str, label: &str, checked: bool, enabled: bool) -> Node {
    node(key, Kind::Toggle, label, checked, enabled, Vec::new())
}

/// The Windows habits' labels that are not a switch's own.
pub mod win_text {
    pub const MENU: &str = "Windows 操作";
    pub const PUNCT_MENU: &str = "Windows 操作：標點";
    pub const CHINESE: &str = "中文輸入（關掉時按鍵直接給 App）";
    pub const GROUP_SWITCH: &str = "── 切換 ──";
    pub const SIDE_BOTH: &str = "左右 Shift 都可以";
    pub const SIDE_LEFT: &str = "只用左 Shift";
    pub const SIDE_RIGHT: &str = "只用右 Shift";
    pub const GROUP_KEYS: &str = "── 按鍵 ──";
    pub const GROUP_MIXED: &str = "── 中英混打 ──";
    pub const MIXED_NOTE: &str = "沒打完的注音照按鍵顯示，按聲調才轉中文；不像注音的就是英文。只支援標準鍵盤。";
    pub const GROUP_LEARNING: &str = "── 學習 ──";
    pub const GROUP_TEMPLATE: &str = "── 範本 ──";
    pub const TEMPLATE: &str = "套用舊版 Windows 操作（也會關掉即時預測與上游中英混打、改成一律聊天）";
    pub const RESET: &str = "Windows 操作全部還原成上游";
    pub const PUNCT_ALL_ON: &str = "全部打中文標點";
    pub const PUNCT_ALL_OFF: &str = "全部改成直接送出按鍵";
    pub const PUNCT_NOTE: &str = "勾選：善解打出中文標點。取消：按鍵直接給 App。";
}

fn win_flag_node(shell: &Shell, k: &str, enabled: bool) -> Node {
    let f = winops::FLAGS.iter().find(|f| f.key == k).expect("a habit of winops::FLAGS");
    toggle(f.key, f.label, shell.win_flag(f.key), enabled)
}

/// Windows 操作 and Windows 操作：標點.
fn win_submenus(shell: &Shell) -> Vec<Node> {
    use win_text as wt;
    let side = shell.shift_side();
    let shift = shell.win_flag(wk::SHIFT_TOGGLE);
    let mixed = shell.mixed_available();
    let main = vec![
        toggle("win.chinese", wt::CHINESE, shell.chinese(), true),
        win_flag_node(shell, wk::FULL_WIDTH, true),
        info("win.group.switch", wt::GROUP_SWITCH),
        win_flag_node(shell, wk::SHIFT_TOGGLE, true),
        radio("win.side.both", wt::SIDE_BOTH, side == ShiftSide::Both, shift),
        radio("win.side.left", wt::SIDE_LEFT, side == ShiftSide::Left, shift),
        radio("win.side.right", wt::SIDE_RIGHT, side == ShiftSide::Right, shift),
        win_flag_node(shell, wk::CTRL_SPACE, true),
        win_flag_node(shell, wk::SHIFT_SPACE, true),
        win_flag_node(shell, wk::CAPS_ENGLISH, true),
        info("win.group.keys", wt::GROUP_KEYS),
        win_flag_node(shell, wk::CTRL_PUNCT, true),
        win_flag_node(shell, wk::SYMBOLS, true),
        win_flag_node(shell, wk::CODE_INPUT, true),
        win_flag_node(shell, wk::ENTER_COMMITS, true),
        info("win.group.mixed", wt::GROUP_MIXED),
        win_flag_node(shell, wk::MIXED, mixed),
        win_flag_node(shell, wk::MIXED_TOLERANCE, mixed && shell.win_flag(wk::MIXED)),
        info("win.mixed.note", wt::MIXED_NOTE),
        info("win.group.learning", wt::GROUP_LEARNING),
        win_flag_node(shell, wk::LEARNING, true),
        win_flag_node(shell, wk::CONTEXT, true),
        info("win.group.template", wt::GROUP_TEMPLATE),
        item("win.template.windows", wt::TEMPLATE),
        item("win.template.upstream", wt::RESET),
    ];
    let mut punct = vec![info("win.punct.note", wt::PUNCT_NOTE), item("win.punct.all.on", wt::PUNCT_ALL_ON), item("win.punct.all.off", wt::PUNCT_ALL_OFF)];
    punct.extend(winops::punct_switches().iter().map(|(k, label)| toggle(k, label, shell.punct_on(k), true)));
    vec![submenu("win", wt::MENU, true, main), submenu("win.punct", wt::PUNCT_MENU, true, punct)]
}

/// The menu for session `id`.
pub fn build(shell: &Shell, id: SessionId) -> Vec<Node> {
    let mut nodes = Vec::new();
    for (n, e) in shell.menu(id).into_iter().enumerate() {
        match e.action {
            None => nodes.push(info(&format!("info.{n}"), &e.title)),
            Some(a @ Action::Layout(_)) => nodes.push(radio(action_key(a), &e.title, e.checked, e.enabled)),
            Some(Action::OpenSettings) => {
                nodes.extend(settings_submenus(shell));
                nodes.extend(win_submenus(shell));
            }
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
        "win.chinese" => {
            if checked.unwrap_or(!shell.chinese()) != shell.chinese() {
                shell.toggle_chinese(id, out);
            }
        }
        "win.side.both" if selects => shell.set_shift_side(ShiftSide::Both, out),
        "win.side.left" if selects => shell.set_shift_side(ShiftSide::Left, out),
        "win.side.right" if selects => shell.set_shift_side(ShiftSide::Right, out),
        "win.template.windows" => shell.apply_windows_template(out),
        "win.template.upstream" => shell.reset_windows_habits(out),
        "win.punct.all.on" => shell.set_all_punct(true, out),
        "win.punct.all.off" => shell.set_all_punct(false, out),
        k if winops::FLAGS.iter().any(|f| f.key == k) => {
            let on = checked.unwrap_or(!shell.win_flag(k));
            if on != shell.win_flag(k) {
                shell.set_win_flag(k, on, out);
            }
        }
        k if k.starts_with(wk::PUNCT) => {
            let on = checked.unwrap_or(!shell.punct_on(k));
            if on != shell.punct_on(k) {
                shell.set_punct(k, on, out);
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
