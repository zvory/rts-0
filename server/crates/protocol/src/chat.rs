use serde::{Deserialize, Serialize};

pub const MAX_CHAT_CHARS: usize = 200;
pub const MAX_CHAT_INPUT_CHARS: usize = MAX_CHAT_CHARS * 4;

/// Largest valid UTF-8 representation of the room task's scalar inspection budget. Rejecting
/// larger strings during deserialization keeps attacker-controlled allocations out of the bounded
/// room-event queue; the room remains responsible for normalization and the 200-scalar result cap.
pub const MAX_CHAT_INPUT_BYTES: usize = MAX_CHAT_INPUT_CHARS * 4;

#[derive(Debug, Clone, Deserialize)]
#[serde(try_from = "String")]
pub struct ChatText(String);

impl TryFrom<String> for ChatText {
    type Error = String;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        if text.len() > MAX_CHAT_INPUT_BYTES {
            return Err(format!(
                "chat text exceeds {MAX_CHAT_INPUT_BYTES} UTF-8 bytes"
            ));
        }
        Ok(Self(text))
    }
}

impl ChatText {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_inner(self) -> String {
        self.0
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatSendPayload {
    pub channel: ChatChannel,
    pub text: ChatText,
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
