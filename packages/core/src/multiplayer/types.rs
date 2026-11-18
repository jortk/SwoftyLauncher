//! The data the session module reports, as the pages render it. Field names are the module's JSON.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::row_ids::Overrides;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Category {
    #[serde(rename = "f1-2026")]
    F12026,
    #[serde(rename = "f1-2025")]
    F12025,
}

impl Category {
    pub fn as_str(self) -> &'static str {
        match self {
            Category::F12026 => "f1-2026",
            Category::F12025 => "f1-2025",
        }
    }
}

impl std::fmt::Display for Category {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Screen {
    MainMenu,
    LanLobby,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    None,
    Host,
    Client,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cover {
    pub step: String,
    pub percent: u8,
}

/// What the game last reported about itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameStatus {
    pub screen: Screen,
    pub in_session: bool,
    pub role: Role,
    #[serde(default)]
    pub configured: bool,
    #[serde(default, rename = "self")]
    pub self_member: u8,
    #[serde(default)]
    pub base: u16,
    #[serde(default)]
    pub cover: Option<Cover>,
    #[serde(default)]
    pub room_failed: Option<String>,
    #[serde(default)]
    pub content: Option<String>,
}

/// The content name of the F1 2026 cars in [`RoomContent::host`].
pub const F1_2026: &str = "f1_2026";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub avatar: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoomSettings {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub public: bool,
    pub max_members: u8,
    pub category: Category,
}

impl Default for RoomSettings {
    fn default() -> Self {
        Self {
            name: None,
            public: true,
            max_members: 20,
            category: Category::F12026,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoomSummary {
    pub code: String,
    pub name: String,
    pub host: User,
    pub members: u32,
    pub max_members: u32,
    pub category: String,
    #[serde(default)]
    pub digest: Option<String>,
    #[serde(default)]
    pub addons: Option<RoomAddons>,
    #[serde(default)]
    pub created_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoomAddons {
    pub count: u32,
    #[serde(default)]
    pub tracks: Vec<String>,
    #[serde(default)]
    pub list: Vec<RoomAddon>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoomAddon {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemberStage {
    Starting,
    Loading,
    InLobby,
}

impl MemberStage {
    pub fn word(self) -> &'static str {
        match self {
            Self::Starting => "Starting the game",
            Self::Loading => "Loading",
            Self::InLobby => "In the lobby",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Step {
    SigningIn,
    Joining,
    Connecting,
    StartingGame,
    Loading,
    InLobby,
}

/// The one step a session is at, with a percentage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Progress {
    pub step: Step,
    pub label: String,
    pub percent: u8,
}

/// Something the player has to act on, as one plain sentence and whether trying again can help.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Problem {
    pub text: String,
    pub retry: bool,
    /// One more sentence on what is wrong, shown under the text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "phase", content = "reason", rename_all = "snake_case")]
pub enum Phase {
    Connecting(String),
    WaitingForGame(String),
    InLobby,
    Racing,
    Ended(String),
    Failed(String),
}

impl Phase {
    pub fn is_over(&self) -> bool {
        matches!(self, Phase::Ended(_) | Phase::Failed(_))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoomInfo {
    pub code: String,
    pub name: String,
    pub public: bool,
    pub category: String,
    pub max_members: u32,
    pub started: bool,
    pub host: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrintMatch {
    Unknown,
    Matches,
    Differs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelayStatus {
    None,
    Pending,
    Ready,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemberStats {
    pub packets_out: u64,
    pub bytes_out: u64,
    pub packets_in: u64,
    pub bytes_in: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemberState {
    pub id: String,
    pub name: String,
    pub avatar: Option<String>,
    pub number: u8,
    pub ip: String,
    pub host: bool,
    pub me: bool,
    pub connected: bool,
    pub fingerprint: PrintMatch,
    pub relay: RelayStatus,
    pub stage: Option<MemberStage>,
    pub traffic: Option<MemberStats>,
}

/// The module asks the launcher to start the game for the session.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LaunchAsk {
    /// Changes with each ask, so the launcher acts once on each.
    pub seq: u32,
    /// What the build borrows from the host for this session.
    #[serde(default)]
    pub row_ids: Option<Overrides>,
    pub id_addons: usize,
    pub setting_addons: usize,
    /// The game is running but isn't what the session needs: the launcher closes it first.
    pub restart: bool,
    pub why: String,
    /// The direct start request the game reads at boot.
    #[serde(default)]
    pub direct_start: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoomContent {
    pub host: Option<String>,
    pub owns_f1_2026: BTreeMap<String, bool>,
}

impl RoomContent {
    pub fn needs_f1_2026(&self) -> bool {
        self.host.as_deref() == Some(F1_2026)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionState {
    pub phase: Phase,
    pub host: bool,
    pub room: Option<RoomInfo>,
    pub self_member: Option<u8>,
    pub members: Vec<MemberState>,
    /// How this player's content differs from the host's, as the module reports it.
    pub mismatches: Vec<Value>,
    pub lobby_connected: bool,
    pub relay: Option<String>,
    pub base: Option<u16>,
    /// Traffic counters, as the module reports them.
    pub bridge: Option<Value>,
    pub game: Option<GameStatus>,
    pub last_error: Option<String>,
    pub launch: Option<LaunchAsk>,
    pub content: RoomContent,
    /// The step and the problem the module derives from this state; not part of the state's JSON.
    #[serde(skip)]
    pub progress: Option<Progress>,
    #[serde(skip)]
    pub problem: Option<Problem>,
}

impl SessionState {
    pub fn new(host: bool) -> Self {
        Self {
            phase: Phase::Connecting("starting".into()),
            host,
            room: None,
            self_member: None,
            members: Vec::new(),
            mismatches: Vec::new(),
            lobby_connected: false,
            relay: None,
            base: None,
            bridge: None,
            game: None,
            last_error: None,
            launch: None,
            content: RoomContent::default(),
            progress: None,
            problem: None,
        }
    }

    /// The step the module reported with this state.
    pub fn step(&self) -> Progress {
        self.progress.clone().unwrap_or(Progress {
            step: Step::Connecting,
            label: "Connecting".into(),
            percent: 0,
        })
    }
}

/// One step of bringing this player's content in line with the host's.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum FixStep {
    Install {
        id: String,
        name: String,
        version: semver::Version,
        sha256: Option<String>,
    },
    Disable {
        id: String,
        name: String,
    },
    SetOption {
        addon: String,
        option: String,
        value: Option<Value>,
    },
    Reorder {
        order: Vec<String>,
    },
    Manual {
        line: String,
    },
}
