//! Shell.swift's `chatApps` and Learning.swift's `learningDenylist` as Windows sees apps: the executable's file name,
//! lower case (what the TSF DLL sends in `Hello`). Each entry names the macOS bundle ID it stands for; an app with no
//! Windows build has no entry.
use shanjie_shell::PlatformTables;

pub fn tables() -> PlatformTables {
    PlatformTables {
        chat_apps: [
            "discord.exe",        // com.hnc.Discord
            "slack.exe",          // com.tinyspeck.slackmacgap
            "telegram.exe",       // ru.keepcoder.Telegram, com.tdesktop.Telegram
            "line.exe",           // jp.naver.line.mac
            "whatsapp.exe",       // net.whatsapp.WhatsApp (the Store app)
            "whatsapp.root.exe",
            "messenger.exe",      // com.facebook.archon
            // com.apple.MobileSMS (Messages) has no Windows build.
        ]
        .map(String::from)
        .to_vec(),
        learning_denylist: [
            "1password.exe",      // com.1password.1password, com.agilebits.onepassword7
            "bitwarden.exe",      // com.bitwarden.desktop
            "keepassxc.exe",      // org.keepassxc.keepassxc
            "keepass.exe",
            // com.apple.keychainaccess and com.apple.Passwords: Windows' Credential Manager is a Control Panel page, not an
            // app of its own.
        ]
        .map(String::from)
        .to_vec(),
    }
}
