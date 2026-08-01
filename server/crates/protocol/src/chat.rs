use serde::{Deserialize, Serialize};

pub const MAX_CHAT_CHARS: usize = 200;
pub const MAX_CHAT_INPUT_CHARS: usize = MAX_CHAT_CHARS * 4;

/// Largest valid UTF-8 representation of the room task's scalar inspection budget. Rejecting
/// larger strings during deserialization keeps attacker-controlled allocations out of the bounded
/// room-event queue; the room remains responsible for normalization and the 200-scalar result cap.
pub const MAX_CHAT_INPUT_BYTES: usize = MAX_CHAT_INPUT_CHARS * 4;

pub(crate) fn deserialize_chat_text<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let text = String::deserialize(deserializer)?;
    if text.len() > MAX_CHAT_INPUT_BYTES {
        return Err(serde::de::Error::custom(format_args!(
            "chat text exceeds {MAX_CHAT_INPUT_BYTES} UTF-8 bytes"
        )));
    }
    Ok(text)
}

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
