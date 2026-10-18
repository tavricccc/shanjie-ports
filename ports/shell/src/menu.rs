//! The input method menu and the clear's windows (macos/Sources/ShanjieKit/Learning.swift `MenuEntry`, `DialogText`):
//! the same entries in the same order with the same fixed strings. Platforms draw it with their own menu.
//!
//! Left out because the platform has nothing to do it with (ports/docs/inventory.md lists them):
//! - 不要備份選字記憶 (Time Machine's exclusion flag);
//! - the update entry (Sparkle; the ports have no updater).

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputMode {
    Standard,
    Eten,
}

impl InputMode {
    pub fn layout(self) -> u32 {
        match self {
            InputMode::Standard => 0,
            InputMode::Eten => 1,
        }
    }

    pub fn raw(self) -> &'static str {
        match self {
            InputMode::Standard => "standard",
            InputMode::Eten => "eten",
        }
    }

    pub fn from_raw(s: &str) -> Option<InputMode> {
        match s {
            "standard" => Some(InputMode::Standard),
            "eten" => Some(InputMode::Eten),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Layout(InputMode),
    Clear,
    ToggleDemote,
    TogglePrediction,
    ToggleEnglish,
    ToggleAbbreviation,
    ToggleRerank,
    ToggleAcgPack,
    ToggleAppChat,
    OpenSettings,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MenuEntry {
    pub title: String,
    pub action: Option<Action>,
    pub checked: bool,
    /// False: shown greyed (the checkmark still shows the setting).
    pub enabled: bool,
}

impl MenuEntry {
    pub fn new(title: &str, action: Option<Action>, checked: bool, enabled: bool) -> MenuEntry {
        MenuEntry { title: title.to_string(), action, checked, enabled }
    }
}

/// `MenuEntry.Text`.
pub mod text {
    pub const PAUSED_SECURE: &str = "學習已暫停（安全輸入）";
    pub const PAUSED_APP: &str = "學習已暫停（此 App）";
    pub const STANDARD: &str = "標準鍵盤";
    pub const ETEN: &str = "倚天鍵盤";
    pub const DEMOTE: &str = "避免把敏感字詞排在前面";
    pub const PREDICTION: &str = "即時預測";
    pub const ENGLISH: &str = "中英混打（實驗）";
    pub const ENGLISH_DIGITS_NOTE: &str = "打英文不用切換輸入法；數字列的鍵也會打出數字（例如 2026年、3.14），由前文判斷是數字還是注音。";
    pub const ENGLISH_DISPLAY: &str = "英文的顯示";
    pub const ENGLISH_DISPLAY_TYPED: &str = "打字時就顯示";
    pub const ENGLISH_DISPLAY_SPACE: &str = "按空白才顯示";
    pub const ABBREVIATION: &str = "可省略韻母";
    pub const RERANK: &str = "語意校正";
    pub const ACG_PACK: &str = "動漫與遊戲詞";
    pub fn acg_data_date(date: &str) -> String {
        format!("資料更新至 {date}（維基百科）")
    }
    pub const APP_CHAT: &str = "這個 App 用聊天設定";
    pub const SETTINGS: &str = "善解設定…";
    pub const CLEAR: &str = "清除選字記憶…";
    pub const UNAVAILABLE: &str = "選字記憶無法存檔";
}

/// `DialogText`.
pub mod dialog {
    pub const CLEAR_TITLE: &str = "要清除選字記憶嗎？";
    pub const CLEAR_MESSAGE: &str = "會刪除這台電腦上記住的改選、預測列選取，以及從逐字改選學到的新詞，之後要重新學。清除只刪本機檔案；已經進 Time Machine 備份或本機快照的副本不受影響。";
    pub const CLEAR_BUTTON: &str = "清除";
    pub const CANCEL: &str = "取消";
    pub const FAILED_TITLE: &str = "清除選字記憶失敗";
    pub const FAILED_MESSAGE: &str = "有檔案沒有刪掉，請再試一次。";
    pub const OK: &str = "好";
}
