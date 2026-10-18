//! Port of macos/Sources/ShanjieKit/Shell.swift (`Shell`, `Session`) and the shell half of Learning.swift: the one
//! engine, the one candidate panel, which session owns the composition, the order of every core call, the settings and
//! what each setting does to the engine. Method names follow the Swift ones; each says which Swift method it ports.
//!
//! What differs from Swift is only how a platform plugs in:
//! - A session is a `SessionId`. What the Swift shell asks a client (`bundleIdentifier`, the text before the insertion
//!   point, secure input) the platform keeps current in a `ClientInfo` (`Shell::set_client`).
//! - What the Swift shell does to a client or the panel (`insertText`, `setMarkedText`, `panel.show`, ...) is returned
//!   as `Effect`s tagged with the session, in the order Swift makes the calls. The platform performs them in order.
//! - Rerank jobs go to the platform's `RerankRunner`, which calls `Shell::rerank_finished` on the shell's thread.
//! - The clear's windows are `Effect::ConfirmClear` / `Effect::ClearFailed`; the platform answers with
//!   `Shell::clear_learning`.
use crate::core::{CoreEngine, CoreOutput, CoreResult, RerankJob, NO_CHANGE};
use crate::keys::{self, KeyInput, ShanjieKey};
use crate::menu::{text, Action, InputMode, MenuEntry};
use crate::prefs::{key, Defaults, Prefs};
use crate::spacing;
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

pub type SessionId = u64;

pub const CHAT: u32 = 0;
pub const FORMAL: u32 = 1;

/// Shell.swift `ProfileMode`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProfileMode {
    Auto,
    Chat,
    Formal,
}

impl ProfileMode {
    pub fn raw(self) -> &'static str {
        match self {
            ProfileMode::Auto => "auto",
            ProfileMode::Chat => "chat",
            ProfileMode::Formal => "formal",
        }
    }

    pub fn from_raw(s: &str) -> Option<ProfileMode> {
        match s {
            "auto" => Some(ProfileMode::Auto),
            "chat" => Some(ProfileMode::Chat),
            "formal" => Some(ProfileMode::Formal),
            _ => None,
        }
    }
}

/// Shell.swift `RerankLevel`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RerankLevel {
    Conservative,
    Standard,
    Aggressive,
}

impl RerankLevel {
    pub fn code(self) -> u32 {
        match self {
            RerankLevel::Conservative => 0,
            RerankLevel::Standard => 1,
            RerankLevel::Aggressive => 2,
        }
    }

    pub fn raw(self) -> &'static str {
        match self {
            RerankLevel::Conservative => "conservative",
            RerankLevel::Standard => "standard",
            RerankLevel::Aggressive => "aggressive",
        }
    }

    pub fn from_raw(s: &str) -> Option<RerankLevel> {
        match s {
            "conservative" => Some(RerankLevel::Conservative),
            "standard" => Some(RerankLevel::Standard),
            "aggressive" => Some(RerankLevel::Aggressive),
            _ => None,
        }
    }
}

/// Learning.swift `LearningPause`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LearningPause {
    SecureInput,
    App,
}

/// The text before the insertion point a client reported, for the left context (Learning.swift `leftContext()`).
#[derive(Clone, Default)]
pub struct LeftText {
    /// At most `LEFT_CONTEXT_UTF16` UTF-16 units ending at the insertion point.
    pub text: String,
    /// The request started after the start of the document, so the first grapheme may be cut.
    pub truncated: bool,
}

/// What the platform knows about one client now. Kept current with `Shell::set_client`.
#[derive(Clone, Default)]
pub struct ClientInfo {
    /// The bundle ID's role: what the chat apps, the learning denylist and the per-app choices are keyed by. `None` when
    /// the platform cannot tell (Swift: `bundleIdentifier` nil).
    pub app_id: Option<String>,
    /// The macOS shell asks the system (`IsSecureEventInputEnabled`); a platform reports a password field here.
    pub secure_input: bool,
    /// `None`: no insertion point, the client still holds marked text, or the platform cannot read it.
    pub left: Option<LeftText>,
}

/// One call the Swift shell makes on a client or on the panel.
#[derive(Clone, PartialEq)]
pub enum Effect {
    /// `client.insertText`.
    Insert(String),
    /// `client.setMarkedText` with a non-empty preedit: single underline, a thick one over the English run, caret at
    /// `cursor_utf16`.
    Marked { text: String, cursor_utf16: u32, run_start: u32, run_len: u32 },
    /// `client.setMarkedText("")`.
    ClearMarked,
    /// `panel.show`. `anchor_utf16`: the cursor the panel is placed under (Swift asks `lineRect(cursor:)`).
    ShowCandidates {
        candidates: Vec<String>,
        notes: Vec<Option<String>>,
        selected: i32,
        columns: u32,
        first: u32,
        total: u32,
        vertical: u32,
        anchor_utf16: u32,
    },
    /// `panel.showHint`, anchored at the start of the English run.
    ShowHint { text: String, vertical: u32, anchor_utf16: u32 },
    /// `panel.hide`.
    HideCandidates,
    /// `dialogs.confirmClear`: the platform asks, and calls `Shell::clear_learning` only when the user chose 清除.
    ConfirmClear,
    /// `dialogs.clearFailed`.
    ClearFailed,
    /// `NotificationCenter.post(didChangeSettings)`: a settings view re-reads `SettingsState`.
    SettingsChanged,
    /// `onOpenSettings`.
    OpenSettings,
}

pub type Effects = Vec<(SessionId, Effect)>;

/// Not a session: effects that belong to no client (the settings view, the clear's windows opened from it).
pub const NO_SESSION: SessionId = u64::MAX;

/// Runs one rerank job off the shell's thread, then hands it back with `job.run()`'s code to `Shell::rerank_finished` on
/// the shell's thread (Shell.swift `RerankRunner`).
pub trait RerankRunner {
    fn run(&self, ticket: u64, job: RerankJob);
}

/// The fixed lists of Shell.swift and Learning.swift, in the platform's own identifiers.
#[derive(Clone, Default)]
pub struct PlatformTables {
    /// `Shell.chatApps`.
    pub chat_apps: Vec<String>,
    /// `Shell.learningDenylist`.
    pub learning_denylist: Vec<String>,
}

/// Where the shell finds its files.
pub struct Setup {
    /// The data folder (`ports/data/assemble.sh`): the app's Resources.
    pub resources: PathBuf,
    /// Where the core keeps learning.tsv; `None` for none.
    pub learning_directory: Option<PathBuf>,
    pub tables: PlatformTables,
    /// The macOS app passes false (可省略韻母 does not ship, main.swift).
    pub shows_abbreviation: bool,
    /// The system punctuation table (s3e), `None` when the platform has none: the core keeps its built-in list.
    pub punctuation: Option<String>,
    /// Punctuation names (s3f), empty when the platform has none.
    pub names: HashMap<String, String>,
}

/// The settings window's values (Settings.swift `SettingsModel`), read from the shell's cached values.
#[derive(Clone, Debug, PartialEq)]
pub struct SettingsState {
    pub layout: InputMode,
    pub prediction: bool,
    pub abbreviation: bool,
    pub shows_abbreviation: bool,
    pub english: bool,
    pub english_enabled: bool,
    pub english_inline: bool,
    pub english_inline_enabled: bool,
    /// 中英文之間自動加空格 (cjk-spacing section 2).
    pub spacing: bool,
    pub demote: bool,
    pub rerank: bool,
    pub rerank_enabled: bool,
    pub rerank_level: RerankLevel,
    pub rerank_level_enabled: bool,
    pub acg_pack: bool,
    pub acg_data_line: Option<String>,
    pub glass_tint: f64,
    pub candidate_vertical: bool,
    pub profile_mode: ProfileMode,
    /// (app id, true for 聊天), ordered by app id.
    pub app_profiles: Vec<(String, bool)>,
    pub unavailable: bool,
}

struct Pending {
    job: RerankJob,
    session: SessionId,
    engine: u64,
}

pub struct Shell {
    resources: PathBuf,
    punctuation: Option<String>,
    names: HashMap<String, String>,
    prefs: Prefs,
    tables: PlatformTables,
    demote_on: bool,
    prediction_on: bool,
    abbreviation_on: bool,
    shows_abbreviation: bool,
    /// cjk-spacing: a space between Han and ASCII letters or digits in the text the shell commits.
    spacing_on: bool,
    /// cjk-spacing section 1, Swift's `Session.spacingPrev`: per session, the kind of the character before the cursor as
    /// far as the shell knows it; absent when unknown. Never logged.
    spacing_prev: HashMap<SessionId, spacing::Kind>,
    english_on: bool,
    english_inline_on: bool,
    english_available: bool,
    acg_pack_on: bool,
    acg_data_date: Option<String>,
    glass_tint: f64,
    candidate_vertical: bool,
    rerank_on: bool,
    rerank_level: RerankLevel,
    rerank_levels: u32,
    rerank_available: bool,
    rerank_runner: Option<Box<dyn RerankRunner>>,
    rerank_running: Option<(u64, SessionId, u64)>,
    rerank_pending: Option<Pending>,
    next_ticket: u64,
    profile_mode: ProfileMode,
    app_profiles: BTreeMap<String, u32>,
    shown_vertical: u32,
    hint_shown: bool,
    /// Which session the panel is showing for (the platform's panel may be per client).
    panel_session: Option<SessionId>,
    engine: Option<CoreEngine>,
    /// Bumped by every `build()`: a rerank job is applied only to the engine it was taken from (Swift compares references).
    engine_generation: u64,
    mode: InputMode,
    profile: u32,
    learning_directory: Option<PathBuf>,
    learning_open_failed: bool,
    owner: Option<SessionId>,
    composing: bool,
    clients: HashMap<SessionId, ClientInfo>,
}

/// `LeftContext.maxUTF16`.
pub const LEFT_CONTEXT_UTF16: usize = 32;

/// Mirrors `is_han` in core/src/learn.rs (Learning.swift `LeftContext.isHan`).
pub fn is_han(c: char) -> bool {
    matches!(c as u32, 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xF900..=0xFAFF | 0x20000..=0x2A6DF | 0x2A700..=0x2EBEF
        | 0x30000..=0x3134F | 0x2F800..=0x2FA1F | 0x3007)
}

/// Learning.swift `LeftContext.han`: the trailing ≤ 2 consecutive Han characters, `None` when there are none.
/// `truncated` drops the first character, which may be half of a cut cluster.
pub fn han_tail(text: &str, truncated: bool) -> Option<String> {
    let mut chars: Vec<char> = text.chars().collect();
    if truncated && !chars.is_empty() {
        chars.remove(0);
    }
    let tail: Vec<char> = chars.iter().rev().take_while(|c| is_han(**c)).take(2).copied().collect();
    (!tail.is_empty()).then(|| tail.into_iter().rev().collect())
}

/// The last `LEFT_CONTEXT_UTF16` UTF-16 units of `before` (the text from the document start to the insertion point), as
/// Learning.swift requests them: `truncated` when the document had more.
pub fn left_text(before: &str) -> LeftText {
    let units: Vec<u16> = before.encode_utf16().collect();
    let start = units.len().saturating_sub(LEFT_CONTEXT_UTF16);
    LeftText { text: String::from_utf16_lossy(&units[start..]), truncated: start > 0 }
}

/// `latest_source_revision` of acg.json as a UTC "yyyy-MM-dd" (Shell.swift `readAcgDataDate`).
pub fn read_acg_data_date(path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    let at = text.find("\"latest_source_revision\"")?;
    let rest = &text[at + "\"latest_source_revision\"".len()..];
    let rest = rest.trim_start().strip_prefix(':')?.trim_start().strip_prefix('"')?;
    let stamp = &rest[..rest.find('"')?];
    // ISO 8601 in UTC ("2026-10-06T00:08:14Z"): the date is the first ten characters.
    let ok = stamp.len() >= 20
        && stamp.ends_with('Z')
        && stamp.as_bytes()[4] == b'-'
        && stamp.as_bytes()[7] == b'-'
        && stamp.as_bytes()[10] == b'T'
        && stamp[..4].bytes().chain(stamp[5..7].bytes()).chain(stamp[8..10].bytes()).all(|b| b.is_ascii_digit());
    ok.then(|| stamp[..10].to_string())
}

impl Shell {
    /// Shell.swift `init`: every setting read once (stored value, else the default), then the one engine is built.
    pub fn new(setup: Setup, prefs: Prefs, defaults: &Defaults) -> Shell {
        let mode = prefs
            .string(key::LAYOUT)
            .and_then(|s| InputMode::from_raw(&s))
            .or_else(|| InputMode::from_raw(&defaults.layout))
            .unwrap_or(InputMode::Standard);
        let shows_abbreviation = setup.shows_abbreviation;
        let mut shell = Shell {
            acg_data_date: read_acg_data_date(&setup.resources.join("packs/acg.json")),
            resources: setup.resources,
            punctuation: setup.punctuation,
            names: setup.names,
            tables: setup.tables,
            demote_on: prefs.bool(key::DEMOTE).unwrap_or(defaults.demote),
            prediction_on: prefs.bool(key::PREDICTION).unwrap_or(defaults.prediction),
            abbreviation_on: shows_abbreviation && prefs.bool(key::ABBREVIATION).unwrap_or(defaults.abbreviation),
            shows_abbreviation,
            spacing_on: prefs.bool(key::CJK_SPACING).unwrap_or(defaults.cjk_spacing),
            spacing_prev: HashMap::new(),
            english_on: prefs.bool(key::ENGLISH).unwrap_or(defaults.english),
            english_inline_on: prefs.bool(key::ENGLISH_INLINE).unwrap_or(defaults.english_inline),
            english_available: false,
            acg_pack_on: prefs.bool(key::ACG_PACK).unwrap_or(defaults.acg_pack),
            glass_tint: prefs.double(key::GLASS_TINT).unwrap_or(defaults.glass_tint),
            candidate_vertical: prefs.bool(key::CANDIDATE_VERTICAL).unwrap_or(defaults.candidate_vertical),
            rerank_on: prefs.bool(key::RERANK).unwrap_or(defaults.rerank),
            rerank_level: prefs
                .string(key::RERANK_LEVEL)
                .and_then(|s| RerankLevel::from_raw(&s))
                .or_else(|| RerankLevel::from_raw(&defaults.rerank_level))
                .unwrap_or(RerankLevel::Standard),
            rerank_levels: 0,
            rerank_available: false,
            rerank_runner: None,
            rerank_running: None,
            rerank_pending: None,
            next_ticket: 1,
            profile_mode: prefs
                .string(key::PROFILE_MODE)
                .and_then(|s| ProfileMode::from_raw(&s))
                .or_else(|| ProfileMode::from_raw(&defaults.profile_mode))
                .unwrap_or(ProfileMode::Auto),
            app_profiles: parse_app_profiles(&prefs.app_profiles()),
            shown_vertical: 0,
            hint_shown: false,
            panel_session: None,
            engine: None,
            engine_generation: 0,
            mode,
            profile: CHAT,
            learning_directory: setup.learning_directory,
            learning_open_failed: false,
            owner: None,
            composing: false,
            clients: HashMap::new(),
            prefs,
        };
        shell.build();
        shell
    }

    pub fn set_rerank_runner(&mut self, runner: Box<dyn RerankRunner>) {
        self.rerank_runner = Some(runner);
    }

    pub fn has_engine(&self) -> bool {
        self.engine.is_some()
    }

    pub fn composing(&self) -> bool {
        self.composing
    }

    pub fn owner(&self) -> Option<SessionId> {
        self.owner
    }

    pub fn layout(&self) -> InputMode {
        self.mode
    }

    /// Whether the vertical candidate window is on screen (Page Up / Down reach the core only then).
    pub fn vertical_open(&self) -> bool {
        self.shown_vertical == 1
    }

    /// Whether the hint, not a candidate list, is what the panel shows (a click on it is not a pick).
    pub fn hint_shown(&self) -> bool {
        self.hint_shown
    }

    /// The platform's current knowledge of a client; call before the session's next event.
    pub fn set_client(&mut self, id: SessionId, info: ClientInfo) {
        self.clients.insert(id, info);
    }

    fn client(&self, id: SessionId) -> ClientInfo {
        self.clients.get(&id).cloned().unwrap_or_default()
    }

    fn changed(&self, out: &mut Effects) {
        out.push((NO_SESSION, Effect::SettingsChanged));
    }

    // MARK: Shell.swift

    /// `Shell.profile(bundle:mode:overrides:)`.
    fn profile_for_app(&self, app: Option<&str>) -> u32 {
        match self.profile_mode {
            ProfileMode::Chat => CHAT,
            ProfileMode::Formal => FORMAL,
            ProfileMode::Auto => match app {
                None | Some("") => FORMAL,
                Some(b) => self.app_profiles.get(b).copied().unwrap_or(if self.tables.chat_apps.iter().any(|c| c == b) { CHAT } else { FORMAL }),
            },
        }
    }

    /// `selectLayout`.
    pub fn select_layout(&mut self, new_mode: InputMode, out: &mut Effects) {
        self.switch_mode(new_mode, out);
        self.prefs.set_string(key::LAYOUT, new_mode.raw());
        self.changed(out);
    }

    fn drop_on_failure(&mut self, r: Option<CoreResult>, out: &mut Effects) {
        if let Some(CoreResult::Failed(_)) = r {
            self.discard_orphan(out);
        }
    }

    /// `applyDemote` and the other `apply*` of the settings window: through the owner when there is one.
    pub fn apply_demote(&mut self, on: bool, out: &mut Effects) {
        match self.owner {
            Some(o) => self.session_apply_setting(o, |s| s.set_demote(on), out, false),
            None => {
                let r = self.set_demote(on);
                self.changed(out);
                self.drop_on_failure(r, out)
            }
        }
    }

    pub fn apply_prediction(&mut self, on: bool, out: &mut Effects) {
        match self.owner {
            Some(o) => self.session_apply_setting(o, |s| s.set_prediction(on), out, false),
            None => {
                let r = self.set_prediction(on);
                self.changed(out);
                self.drop_on_failure(r, out)
            }
        }
    }

    pub fn apply_abbreviation(&mut self, on: bool, out: &mut Effects) {
        match self.owner {
            Some(o) => self.session_apply_setting(o, |s| s.set_abbreviation(on), out, false),
            None => {
                let r = self.set_abbreviation(on);
                if self.shows_abbreviation {
                    self.changed(out);
                }
                self.drop_on_failure(r, out)
            }
        }
    }

    /// `setSpacing`: the shell's own switch, nothing goes to the core.
    pub fn set_spacing(&mut self, on: bool, out: &mut Effects) {
        self.spacing_on = on;
        self.prefs.set_bool(key::CJK_SPACING, on);
        self.changed(out);
    }

    pub fn apply_english(&mut self, on: bool, out: &mut Effects) {
        match self.owner {
            Some(o) => self.session_apply_setting(o, |s| s.set_english(on), out, false),
            None => {
                let r = self.set_english(on);
                self.changed(out);
                self.drop_on_failure(r, out)
            }
        }
    }

    pub fn apply_candidate_vertical(&mut self, on: bool, out: &mut Effects) {
        match self.owner {
            Some(o) => self.session_apply_setting(o, |s| s.set_candidate_vertical(on), out, true),
            None => {
                let r = self.set_candidate_vertical(on);
                self.changed(out);
                self.drop_on_failure(r, out)
            }
        }
    }

    pub fn apply_english_inline(&mut self, on: bool, out: &mut Effects) {
        match self.owner {
            Some(o) => self.session_apply_setting(o, |s| s.set_english_inline(on), out, false),
            None => {
                let r = self.set_english_inline(on);
                self.changed(out);
                self.drop_on_failure(r, out)
            }
        }
    }

    pub fn apply_rerank(&mut self, on: bool, out: &mut Effects) {
        match self.owner {
            Some(o) => self.session_apply_setting(o, |s| s.set_rerank(on), out, false),
            None => {
                let r = self.set_rerank(on);
                self.changed(out);
                self.drop_on_failure(r, out)
            }
        }
    }

    pub fn apply_rerank_level(&mut self, level: RerankLevel, out: &mut Effects) {
        match self.owner {
            Some(o) => self.session_apply_setting(o, |s| s.set_rerank_level(level), out, false),
            None => {
                let r = self.set_rerank_level(level);
                self.changed(out);
                self.drop_on_failure(r, out)
            }
        }
    }

    /// The Session `apply*` methods: the setter (stores, calls the engine), the change posted (Swift's `defer`), then the
    /// snapshot shown if `id` owns the composition. `vertical_only`: `applyCandidateVertical` shows it only when its
    /// `vertical` differs from what the panel shows.
    fn session_apply_setting(&mut self, id: SessionId, f: impl FnOnce(&mut Shell) -> Option<CoreResult>, out: &mut Effects, vertical_only: bool) {
        let r = f(self);
        self.changed(out);
        match r {
            Some(CoreResult::Ok(o)) if self.owner == Some(id) && (!vertical_only || o.vertical != self.shown_vertical) => self.apply(id, o, out),
            Some(CoreResult::Failed(c)) => {
                self.fail(id, c, out);
            }
            _ => {}
        }
    }

    /// The clear with its confirmation window (`confirmAndClear`).
    pub fn confirm_and_clear(&mut self, id: SessionId, out: &mut Effects) {
        out.push((id, Effect::ConfirmClear));
    }

    /// The platform's answer to `Effect::ConfirmClear`: 清除 (Learning.swift `clearLearning`).
    pub fn clear_learning(&mut self, id: SessionId, out: &mut Effects) {
        let code = self.engine.as_ref().map(|e| e.learning_clear()).unwrap_or(4);
        if code != 0 {
            out.push((id, Effect::ClearFailed));
        }
    }

    /// `setGlassTint`. Stored and posted like on macOS; a platform without glass draws nothing with it.
    pub fn set_glass_tint(&mut self, value: f64, out: &mut Effects) {
        self.glass_tint = value;
        self.prefs.set_double(key::GLASS_TINT, value);
        self.changed(out);
    }

    /// `setProfileMode`.
    pub fn set_profile_mode(&mut self, m: ProfileMode, out: &mut Effects) {
        self.profile_mode = m;
        self.prefs.set_string(key::PROFILE_MODE, m.raw());
        self.changed(out);
        self.reapply_profile_to_owner(out);
    }

    /// `removeAppProfile`.
    pub fn remove_app_profile(&mut self, app: &str, out: &mut Effects) {
        self.app_profiles.remove(app);
        self.store_app_profiles();
        self.changed(out);
        self.reapply_profile_to_owner(out);
    }

    fn reapply_profile_to_owner(&mut self, out: &mut Effects) {
        if let Some(o) = self.owner {
            let app = self.client(o).app_id;
            self.apply_profile(o, app.as_deref(), out);
        }
    }

    /// `setAppProfile`: a value equal to the built-in one removes the entry.
    fn set_app_profile(&mut self, app: &str, p: u32) {
        let builtin = if self.tables.chat_apps.iter().any(|c| c == app) { CHAT } else { FORMAL };
        if p == builtin {
            self.app_profiles.remove(app);
        } else {
            self.app_profiles.insert(app.to_string(), p);
        }
        self.store_app_profiles();
    }

    fn store_app_profiles(&mut self) {
        let stored = self
            .app_profiles
            .iter()
            .map(|(k, v)| (k.clone(), if *v == CHAT { "chat" } else { "formal" }.to_string()))
            .collect();
        self.prefs.set_app_profiles(&stored);
    }

    /// `setDemote`: stored and sent to the engine; the caller posts the change (Swift's `defer`) and shows the snapshot.
    /// The other setters below follow the same pattern.
    fn set_demote(&mut self, on: bool) -> Option<CoreResult> {
        self.demote_on = on;
        self.prefs.set_bool(key::DEMOTE, on);
        self.engine.as_ref().map(|e| e.set_demote(on))
    }

    fn set_prediction(&mut self, on: bool) -> Option<CoreResult> {
        self.prediction_on = on;
        self.prefs.set_bool(key::PREDICTION, on);
        self.engine.as_ref().map(|e| e.set_prediction(on))
    }

    fn set_abbreviation(&mut self, on: bool) -> Option<CoreResult> {
        if !self.shows_abbreviation {
            return None;
        }
        self.abbreviation_on = on;
        self.prefs.set_bool(key::ABBREVIATION, on);
        self.engine.as_ref().map(|e| e.set_abbreviation(on))
    }

    fn set_english(&mut self, on: bool) -> Option<CoreResult> {
        self.english_on = on;
        self.prefs.set_bool(key::ENGLISH, on);
        self.engine.as_ref().map(|e| e.set_english(on))
    }

    fn set_english_inline(&mut self, on: bool) -> Option<CoreResult> {
        self.english_inline_on = on;
        self.prefs.set_bool(key::ENGLISH_INLINE, on);
        self.engine.as_ref().map(|e| e.set_english_inline(on))
    }

    fn set_candidate_vertical(&mut self, on: bool) -> Option<CoreResult> {
        self.candidate_vertical = on;
        self.prefs.set_bool(key::CANDIDATE_VERTICAL, on);
        self.engine.as_ref().map(|e| e.set_candidate_vertical(on))
    }

    /// `setRerank`: a job waiting for the runner is dropped; one running is not applied.
    fn set_rerank(&mut self, on: bool) -> Option<CoreResult> {
        self.rerank_on = on;
        self.prefs.set_bool(key::RERANK, on);
        if !on {
            self.rerank_pending = None;
        }
        self.engine.as_ref().map(|e| e.set_rerank(on))
    }

    fn set_rerank_level(&mut self, level: RerankLevel) -> Option<CoreResult> {
        self.rerank_level = level;
        self.prefs.set_string(key::RERANK_LEVEL, level.raw());
        self.engine.as_ref().map(|e| e.set_rerank_level(level.code()))
    }

    /// `enqueueRerank`.
    fn enqueue_rerank(&mut self, job: RerankJob, session: SessionId) {
        let pending = Pending { job, session, engine: self.engine_generation };
        if self.rerank_running.is_some() {
            self.rerank_pending = Some(pending);
        } else {
            self.start_rerank(pending);
        }
    }

    fn start_rerank(&mut self, p: Pending) {
        let ticket = self.next_ticket;
        self.next_ticket += 1;
        self.rerank_running = Some((ticket, p.session, p.engine));
        match &self.rerank_runner {
            Some(r) => r.run(ticket, p.job),
            // No runner: the job is freed unrun, as if run had failed; nothing is applied.
            None => self.rerank_running = None,
        }
    }

    /// `rerankFinished`: on the shell's thread once run returned.
    pub fn rerank_finished(&mut self, ticket: u64, job: RerankJob, code: i32, out: &mut Effects) {
        let running = self.rerank_running.take();
        if let Some((t, session, engine)) = running.filter(|r| r.0 == ticket) {
            let _ = t;
            if code == 0 && self.rerank_on && self.owner == Some(session) && engine == self.engine_generation && self.engine.is_some() {
                self.apply_rerank_job(session, &job, out);
            }
        }
        drop(job);
        if let Some(next) = self.rerank_pending.take() {
            self.start_rerank(next);
        }
    }

    /// `setAcgPack`.
    pub fn set_acg_pack(&mut self, on: bool, out: &mut Effects) {
        if on == self.acg_pack_on {
            return;
        }
        self.commit_then_rebuild(out, |s| {
            s.acg_pack_on = on;
            s.prefs.set_bool(key::ACG_PACK, on);
        });
        self.changed(out);
    }

    /// `build()`: the engine for the current mode, the LM, the small model, the English list, then every setting.
    fn build(&mut self) {
        self.engine = None;
        self.engine_generation += 1;
        self.rerank_available = false;
        self.rerank_levels = 0;
        self.english_available = false;
        self.rerank_pending = None;
        let res = self.resources.to_string_lossy().into_owned();
        let packs = self.resources.join("packs").to_string_lossy().into_owned();
        let layout = self.mode.layout();
        let mut made = CoreEngine::make(&res, layout, self.acg_pack_on.then_some(packs.as_str()));
        if made.is_err() && self.acg_pack_on {
            made = CoreEngine::make(&res, layout, None);
        }
        let Ok(e) = made else { return };
        let lm = e.load_lm(&self.resources.join("bigram.sjlm").to_string_lossy());
        if lm == 0 {
            let tiny = e.load_tinylm(&self.resources.join("tinylm.sjtl").to_string_lossy());
            self.rerank_available = tiny == 0;
        }
        let english = e.load_english(&self.resources.join("english-words.txt").to_string_lossy());
        self.english_available = english == 0;
        let _ = e.set_demote(self.demote_on);
        let _ = e.set_prediction(self.prediction_on);
        let _ = e.set_abbreviation(self.abbreviation_on);
        let _ = e.set_english(self.english_on);
        let _ = e.set_english_inline(self.english_inline_on);
        let _ = e.set_candidate_vertical(self.candidate_vertical);
        let _ = e.set_rerank(self.rerank_on);
        let _ = e.set_rerank_level(self.rerank_level.code());
        self.rerank_levels = e.rerank_levels();
        let _ = e.set_profile(self.profile);
        if let Some(table) = &self.punctuation {
            let _ = e.set_punctuation(table);
        }
        if let Some(dir) = &self.learning_directory {
            let code = e.learning_open(&dir.to_string_lossy());
            self.learning_open_failed = code != 0;
        }
        self.engine = Some(e);
    }

    /// `switchMode(to:)`.
    fn switch_mode(&mut self, new_mode: InputMode, out: &mut Effects) {
        if new_mode == self.mode {
            return;
        }
        self.commit_then_rebuild(out, |s| s.mode = new_mode);
    }

    /// `commitThenRebuild`.
    fn commit_then_rebuild(&mut self, out: &mut Effects, apply: impl FnOnce(&mut Shell)) {
        if self.composing {
            match self.owner {
                Some(o) => self.finish(o, 0, out),
                None => self.discard_orphan(out),
            }
        }
        apply(self);
        self.build();
    }

    fn set_profile(&mut self, p: u32) -> Option<CoreResult> {
        self.profile = p;
        self.engine.as_ref().map(|e| e.set_profile(p))
    }

    fn show_candidates(&mut self, id: SessionId, o: &CoreOutput, out: &mut Effects) {
        let notes = o.candidates.iter().map(|c| self.names.get(c).cloned()).collect();
        self.shown_vertical = o.vertical;
        self.hint_shown = false;
        self.panel_session = Some(id);
        out.push((
            id,
            Effect::ShowCandidates {
                candidates: o.candidates.clone(),
                notes,
                selected: o.selected,
                columns: o.columns,
                first: o.first,
                total: o.total,
                vertical: o.vertical,
                anchor_utf16: o.cursor_utf16,
            },
        ));
    }

    /// `Shell.englishRunHint`.
    pub const ENGLISH_RUN_HINT: &'static str = "空白：英文斷開";

    fn show_hint(&mut self, id: SessionId, anchor: u32, out: &mut Effects) {
        self.shown_vertical = if self.candidate_vertical { 2 } else { 0 };
        self.hint_shown = true;
        self.panel_session = Some(id);
        out.push((id, Effect::ShowHint { text: Self::ENGLISH_RUN_HINT.to_string(), vertical: self.shown_vertical, anchor_utf16: anchor }));
    }

    /// `hideCandidates`: the one panel, wherever it is.
    fn hide_candidates(&mut self, out: &mut Effects) {
        self.hint_shown = false;
        self.shown_vertical = 0;
        let target = self.panel_session.take().or(self.owner).unwrap_or(NO_SESSION);
        out.push((target, Effect::HideCandidates));
    }

    /// `discardOrphan`.
    fn discard_orphan(&mut self, out: &mut Effects) {
        if let Some(e) = &self.engine {
            let _ = e.reset(1);
        }
        self.composing = false;
        self.hide_candidates(out);
    }

    // MARK: Learning.swift (the Shell extension)

    /// `learningPause(bundle:)`.
    fn learning_pause(&self, info: &ClientInfo) -> Option<LearningPause> {
        if info.secure_input {
            return Some(LearningPause::SecureInput);
        }
        match info.app_id.as_deref() {
            None | Some("") => Some(LearningPause::App),
            Some(b) if self.tables.learning_denylist.iter().any(|d| d == b) => Some(LearningPause::App),
            Some(_) => None,
        }
    }

    pub fn learning_pause_for(&self, id: SessionId) -> Option<LearningPause> {
        self.learning_pause(&self.client(id))
    }

    /// `learningUnavailable`.
    pub fn learning_unavailable(&self) -> bool {
        self.learning_open_failed || self.engine.as_ref().and_then(|e| e.learning_status()).is_some_and(|f| f & 1 != 0)
    }

    // MARK: Session (Shell.swift)

    /// The platform's controller was created: nothing to do until it is used (Swift's `Session.init`).
    pub fn open_session(&mut self, id: SessionId, info: ClientInfo) {
        self.clients.insert(id, info);
    }

    /// Session `deinit`: the controller is gone. If it owned a composition, it is discarded.
    pub fn close_session(&mut self, id: SessionId, out: &mut Effects) {
        if self.owner == Some(id) {
            self.owner = None;
        }
        if self.panel_session == Some(id) {
            self.panel_session = None;
        }
        self.clients.remove(&id);
        self.spacing_prev.remove(&id);
        if self.owner.is_none() && self.composing {
            self.discard_orphan(out);
        }
    }

    /// `Session.handle(_:)`. Returns whether the key was consumed.
    pub fn handle(&mut self, id: SessionId, input: KeyInput, out: &mut Effects) -> bool {
        if self.engine.is_none() {
            return false;
        }
        if self.claim(id, out) {
            let app = self.client(id).app_id;
            self.apply_profile(id, app.as_deref(), out);
        }
        match input {
            KeyInput::Page { up, modifiers } if self.vertical_open() && modifiers == 0 => {
                self.send(id, keys::special(if up { keys::PAGE_UP } else { keys::PAGE_DOWN }, 0), out)
            }
            KeyInput::Mapped(key) => self.send(id, key, out),
            _ => {
                if self.composing {
                    self.finish(id, 0, out);
                }
                false
            }
        }
    }

    /// `Session.activate()`.
    pub fn activate(&mut self, id: SessionId, out: &mut Effects) {
        self.claim(id, out);
        let app = self.client(id).app_id;
        self.apply_profile(id, app.as_deref(), out);
    }

    /// `Session.deactivate()`.
    pub fn deactivate(&mut self, id: SessionId, out: &mut Effects) {
        self.spacing_prev.remove(&id);
        if self.owner != Some(id) {
            return;
        }
        if self.composing {
            let secure = self.client(id).secure_input;
            self.finish(id, if secure { 1 } else { 0 }, out);
        }
        self.hide_candidates(out);
    }

    /// `Session.commitComposition()`.
    pub fn commit_composition(&mut self, id: SessionId, out: &mut Effects) {
        if self.owner != Some(id) {
            return;
        }
        if self.composing {
            self.finish(id, 0, out);
        }
        self.hide_candidates(out);
    }

    /// `Session.candidateSelected(at:)`: a mouse click on a candidate. Returns whether a pick was sent to the core.
    pub fn candidate_selected(&mut self, id: SessionId, index: u32, out: &mut Effects) -> bool {
        if self.owner != Some(id) || self.engine.is_none() || self.hint_shown {
            return false;
        }
        self.apply_learning_gate(id);
        let r = self.engine.as_ref().map(|e| e.pick(index));
        match r {
            Some(CoreResult::Ok(o)) => self.apply(id, o, out),
            Some(CoreResult::Failed(2)) => {}
            Some(CoreResult::Failed(c)) => {
                self.fail(id, c, out);
            }
            None => {}
        }
        true
    }

    /// `Session.claim()`. Returns whether the owner changed.
    fn claim(&mut self, id: SessionId, out: &mut Effects) -> bool {
        if self.owner == Some(id) {
            return false;
        }
        if self.composing {
            match self.owner {
                Some(previous) => self.finish(previous, 0, out),
                None => self.discard_orphan(out),
            }
        }
        if let Some(previous) = self.owner {
            self.spacing_prev.remove(&previous);
        }
        self.spacing_prev.remove(&id);
        self.owner = Some(id);
        true
    }

    /// `Session.applyRerankJob`.
    fn apply_rerank_job(&mut self, id: SessionId, job: &RerankJob, out: &mut Effects) {
        let r = match &self.engine {
            Some(e) => e.rerank_apply(job),
            None => return,
        };
        match r {
            CoreResult::Ok(o) => self.apply(id, o, out),
            CoreResult::Failed(c) if c == NO_CHANGE => {}
            CoreResult::Failed(c) => {
                self.fail(id, c, out);
            }
        }
    }

    /// `Session.takeRerank()`.
    fn take_rerank(&mut self, id: SessionId, out: &mut Effects) {
        let taken = match &self.engine {
            Some(e) => e.rerank_take(),
            None => return,
        };
        match taken {
            Err(code) => {
                self.fail(id, code, out);
            }
            Ok(Some(job)) => self.enqueue_rerank(job, id),
            Ok(None) => {}
        }
    }

    /// `Session.toggleAppChat()`.
    fn toggle_app_chat(&mut self, id: SessionId, out: &mut Effects) {
        let Some(app) = self.client(id).app_id.filter(|a| !a.is_empty()) else { return };
        if self.profile_mode != ProfileMode::Auto {
            return;
        }
        let next = if self.profile_for_app(Some(&app)) == CHAT { FORMAL } else { CHAT };
        self.set_app_profile(&app, next);
        self.claim(id, out);
        self.apply_profile(id, Some(&app), out);
        self.changed(out);
    }

    /// `Session.applyProfile(bundle:)`.
    fn apply_profile(&mut self, id: SessionId, app: Option<&str>, out: &mut Effects) {
        let p = self.profile_for_app(app);
        match self.set_profile(p) {
            Some(CoreResult::Ok(o)) => self.apply(id, o, out),
            Some(CoreResult::Failed(c)) => {
                self.fail(id, c, out);
            }
            None => {}
        }
    }

    /// `Session.send(_:)`: the single place a key reaches the core.
    fn send(&mut self, id: SessionId, key: ShanjieKey, out: &mut Effects) -> bool {
        if self.engine.is_none() {
            return false;
        }
        let paused = self.apply_learning_gate(id);
        // Once per composition, before the key that may start it: only a character key without Control, Option, Command
        // or Caps Lock can start one. cjk-spacing section 1: the previous character starts as unknown for every key that
        // may start a composition, modifiers or not, and is taken from the same read.
        if !self.composing {
            self.spacing_prev.remove(&id);
            if key.kind == keys::CHAR && key.modifiers & !keys::SHIFT == 0 {
                let (han, last) = if paused { (None, None) } else { self.left_context(id) };
                if let Some(k) = last {
                    self.spacing_prev.insert(id, k);
                }
                if let Some(e) = &self.engine {
                    let _ = e.set_left_context(han.as_deref());
                }
            }
        }
        let r = self.engine.as_ref().map(|e| e.key(key));
        match r {
            Some(CoreResult::Ok(o)) => {
                let handled = o.handled;
                // handled 0 with nothing to commit is a pure pass-through: the client is not touched either. The app
                // inserts or moves the cursor by itself then, so the previous character is unknown.
                if o.handled || !o.commit.is_empty() {
                    self.apply(id, o, out);
                }
                if !handled {
                    self.spacing_prev.remove(&id);
                }
                handled
            }
            Some(CoreResult::Failed(c)) => self.fail(id, c, out),
            None => false,
        }
    }

    /// `Session.finish(mode:)`: 0 commits to this session's client, 1 discards.
    fn finish(&mut self, id: SessionId, mode: u32, out: &mut Effects) {
        if self.engine.is_none() {
            return;
        }
        self.apply_learning_gate(id);
        let r = self.engine.as_ref().map(|e| e.reset(mode));
        match r {
            Some(CoreResult::Ok(o)) => self.apply(id, o, out),
            Some(CoreResult::Failed(c)) => {
                self.fail(id, c, out);
            }
            None => {}
        }
    }

    /// `Session.apply(_:)`: commit first, then the preedit, then the candidates (section 7).
    fn apply(&mut self, id: SessionId, o: CoreOutput, out: &mut Effects) {
        if !o.commit.is_empty() {
            let text = if self.spacing_on { spacing::apply(self.spacing_prev.get(&id).copied(), &o.commit) } else { o.commit.clone() };
            if let Some(k) = spacing::last(&text) {
                self.spacing_prev.insert(id, k);
            }
            out.push((id, Effect::Insert(text)));
        }
        if !o.preedit.is_empty() {
            out.push((
                id,
                Effect::Marked { text: o.preedit.clone(), cursor_utf16: o.cursor_utf16, run_start: o.english_run_start, run_len: o.english_run_len },
            ));
        } else if self.composing {
            out.push((id, Effect::ClearMarked));
        }
        self.composing = !o.preedit.is_empty();
        let hint = o.candidates.is_empty() && o.english_run_len > 0;
        if o.candidates.is_empty() && !hint {
            self.hide_candidates(out);
        } else if hint {
            self.show_hint(id, o.english_run_start, out);
        } else {
            self.show_candidates(id, &o, out);
        }
        self.take_rerank(id, out);
    }

    /// `Session.fail(_:)`: the core discards its composition too, the marked text and the panel are cleared, the key passes.
    fn fail(&mut self, id: SessionId, _code: i32, out: &mut Effects) -> bool {
        if let Some(e) = &self.engine {
            let _ = e.reset(1);
        }
        out.push((id, Effect::ClearMarked));
        self.composing = false;
        self.hide_candidates(out);
        false
    }

    // MARK: Session, Learning.swift

    /// `applyLearningGate`: sampled before every core call that may learn, sent every time. Returns whether paused.
    fn apply_learning_gate(&mut self, id: SessionId) -> bool {
        let paused = self.learning_pause(&self.client(id)).is_some();
        if let Some(e) = &self.engine {
            let _ = e.set_learning(!paused);
        }
        paused
    }

    /// `leftContext()`: the trailing Han characters for the core and, from the same read, the kind of the last character
    /// for cjk-spacing.
    fn left_context(&self, id: SessionId) -> (Option<String>, Option<spacing::Kind>) {
        match self.client(id).left {
            Some(left) if !left.text.is_empty() => (han_tail(&left.text, left.truncated), spacing::last(&left.text)),
            _ => (None, None),
        }
    }

    /// `Session.menu`, in order.
    pub fn menu(&self, id: SessionId) -> Vec<MenuEntry> {
        let info = self.client(id);
        let mut items = Vec::new();
        match self.learning_pause(&info) {
            Some(LearningPause::SecureInput) => items.push(MenuEntry::new(text::PAUSED_SECURE, None, false, true)),
            Some(LearningPause::App) => items.push(MenuEntry::new(text::PAUSED_APP, None, false, true)),
            None => {}
        }
        items.push(MenuEntry::new(text::STANDARD, Some(Action::Layout(InputMode::Standard)), self.mode == InputMode::Standard, true));
        items.push(MenuEntry::new(text::ETEN, Some(Action::Layout(InputMode::Eten)), self.mode == InputMode::Eten, true));
        items.push(MenuEntry::new(text::DEMOTE, Some(Action::ToggleDemote), self.demote_on, true));
        items.push(MenuEntry::new(text::PREDICTION, Some(Action::TogglePrediction), self.prediction_on, true));
        if self.shows_abbreviation {
            items.push(MenuEntry::new(text::ABBREVIATION, Some(Action::ToggleAbbreviation), self.abbreviation_on, self.prediction_on));
        }
        items.push(MenuEntry::new(text::ENGLISH, Some(Action::ToggleEnglish), self.english_on, self.english_enabled()));
        items.push(MenuEntry::new(text::RERANK, Some(Action::ToggleRerank), self.rerank_on, self.rerank_enabled()));
        items.push(MenuEntry::new(text::ACG_PACK, Some(Action::ToggleAcgPack), self.acg_pack_on, true));
        if let Some(date) = &self.acg_data_date {
            items.push(MenuEntry::new(&text::acg_data_date(date), None, false, true));
        }
        if let Some(app) = info.app_id.as_deref().filter(|a| !a.is_empty()) {
            items.push(MenuEntry::new(text::APP_CHAT, Some(Action::ToggleAppChat), self.profile_for_app(Some(app)) == CHAT, self.profile_mode == ProfileMode::Auto));
        }
        items.push(MenuEntry::new(text::SETTINGS, Some(Action::OpenSettings), false, true));
        items.push(MenuEntry::new(text::CLEAR, Some(Action::Clear), false, true));
        if self.learning_unavailable() {
            items.push(MenuEntry::new(text::UNAVAILABLE, None, false, true));
        }
        items
    }

    /// `Session.perform(_:)`.
    pub fn perform(&mut self, id: SessionId, action: Action, out: &mut Effects) {
        match action {
            Action::Layout(m) => self.select_layout(m, out),
            Action::Clear => self.confirm_and_clear(id, out),
            Action::ToggleDemote => {
                let on = !self.demote_on;
                self.session_apply_setting(id, |s| s.set_demote(on), out, false);
            }
            Action::TogglePrediction => {
                let on = !self.prediction_on;
                self.session_apply_setting(id, |s| s.set_prediction(on), out, false);
            }
            Action::ToggleEnglish => {
                let on = !self.english_on;
                self.session_apply_setting(id, |s| s.set_english(on), out, false);
            }
            Action::ToggleAbbreviation => {
                let on = !self.abbreviation_on;
                let shows = self.shows_abbreviation;
                let r = self.set_abbreviation(on);
                if shows {
                    self.changed(out);
                }
                match r {
                    Some(CoreResult::Ok(o)) if self.owner == Some(id) => self.apply(id, o, out),
                    Some(CoreResult::Failed(c)) => {
                        self.fail(id, c, out);
                    }
                    _ => {}
                }
            }
            // To the owner of the composition, not this session (Swift: `shell.applyRerank`).
            Action::ToggleRerank => {
                let on = !self.rerank_on;
                self.apply_rerank(on, out);
            }
            Action::ToggleAcgPack => {
                let on = !self.acg_pack_on;
                self.set_acg_pack(on, out);
            }
            Action::ToggleAppChat => self.toggle_app_chat(id, out),
            Action::OpenSettings => out.push((id, Effect::OpenSettings)),
        }
    }

    // MARK: the settings window (Settings.swift)

    fn english_enabled(&self) -> bool {
        self.english_available && self.mode == InputMode::Standard
    }

    fn rerank_enabled(&self) -> bool {
        self.demote_on && self.rerank_available
    }

    pub fn settings(&self) -> SettingsState {
        SettingsState {
            layout: self.mode,
            prediction: self.prediction_on,
            abbreviation: self.abbreviation_on,
            shows_abbreviation: self.shows_abbreviation,
            english: self.english_on,
            english_enabled: self.english_enabled(),
            english_inline: self.english_inline_on,
            english_inline_enabled: self.english_enabled() && self.english_on,
            spacing: self.spacing_on,
            demote: self.demote_on,
            rerank: self.rerank_on,
            rerank_enabled: self.rerank_enabled(),
            rerank_level: self.rerank_level,
            rerank_level_enabled: self.rerank_on && self.rerank_enabled() && self.rerank_levels & 0b101 == 0b101,
            acg_pack: self.acg_pack_on,
            acg_data_line: self.acg_data_date.as_deref().map(text::acg_data_date),
            glass_tint: self.glass_tint,
            candidate_vertical: self.candidate_vertical,
            profile_mode: self.profile_mode,
            app_profiles: self.app_profiles.iter().map(|(k, v)| (k.clone(), *v == CHAT)).collect(),
            unavailable: self.learning_unavailable(),
        }
    }
}

/// `AppProfiles.parse`: entry by entry, only a non-empty id with `chat` or `formal`.
fn parse_app_profiles(raw: &[(String, String)]) -> BTreeMap<String, u32> {
    raw.iter()
        .filter(|(id, _)| !id.is_empty())
        .filter_map(|(id, v)| match v.as_str() {
            "chat" => Some((id.clone(), CHAT)),
            "formal" => Some((id.clone(), FORMAL)),
            _ => None,
        })
        .collect()
}
