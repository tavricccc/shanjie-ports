//! Shell.swift's `chatApps` and Learning.swift's `learningDenylist`, in the identifiers this port sees (`app_id` in
//! `hub.rs`: the program part of the IBus client name). Each entry names the macOS bundle ID it stands for; an app with no
//! Linux build has no entry.
use shanjie_shell::PlatformTables;

pub fn tables() -> PlatformTables {
    PlatformTables {
        chat_apps: [
            "discord",                  // com.hnc.Discord
            "Discord",
            "slack",                    // com.tinyspeck.slackmacgap
            "Slack",
            "telegram-desktop",         // ru.keepcoder.Telegram, com.tdesktop.Telegram
            "org.telegram.desktop",
            "whatsapp-for-linux",       // net.whatsapp.WhatsApp (no official Linux app; the common community one)
            "com.github.eneshecan.WhatsAppForLinux",
            "caprine",                  // com.facebook.archon (Messenger; no official Linux app)
            // jp.naver.line.mac (LINE) and com.apple.MobileSMS (Messages) have no Linux build.
        ]
        .map(String::from)
        .to_vec(),
        learning_denylist: [
            "1password",                // com.1password.1password, com.agilebits.onepassword7
            "bitwarden",                // com.bitwarden.desktop
            "Bitwarden",
            "keepassxc",                // org.keepassxc.keepassxc
            "org.keepassxc.KeePassXC",
            "seahorse",                 // com.apple.keychainaccess (GNOME's keyring manager)
            "org.gnome.seahorse.Application",
            "secrets",                  // com.apple.Passwords (GNOME Secrets)
            "org.gnome.World.Secrets",
        ]
        .map(String::from)
        .to_vec(),
    }
}
