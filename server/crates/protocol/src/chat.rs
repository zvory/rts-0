use serde::{Deserialize, Serialize};

/// Audience selected for one chat message. The room remains authoritative for recipient routing.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ChatChannel {
    All,
    Team,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ChatScope {
    Lobby,
    Game,
}
