use rts_server::protocol::ClientMessage;

pub(crate) fn is_player_activity(message: &ClientMessage) -> bool {
    match message {
        ClientMessage::Ping { .. }
        | ClientMessage::NetReport { .. }
        | ClientMessage::MatchLoadReady { .. } => false,
        ClientMessage::Join { .. }
        | ClientMessage::SetName { .. }
        | ClientMessage::Ready { .. }
        | ClientMessage::Start
        | ClientMessage::SetTeamPreset { .. }
        | ClientMessage::SetTeam { .. }
        | ClientMessage::SetFaction { .. }
        | ClientMessage::AddAi { .. }
        | ClientMessage::SetAiProfile { .. }
        | ClientMessage::RemoveAi { .. }
        | ClientMessage::SetSpectator { .. }
        | ClientMessage::Command { .. }
        | ClientMessage::GiveUp
        | ClientMessage::PauseGame
        | ClientMessage::UnpauseGame
        | ClientMessage::ReturnToLobby
        | ClientMessage::Activity
        | ClientMessage::SetRoomTimeSpeed { .. }
        | ClientMessage::StepRoomTime
        | ClientMessage::SeekRoomTime { .. }
        | ClientMessage::SeekRoomTimeTo { .. }
        | ClientMessage::SetVisionSelection { .. }
        | ClientMessage::Lab { .. }
        | ClientMessage::RequestBranchFromTick
        | ClientMessage::ClaimBranchSeat { .. }
        | ClientMessage::ReleaseBranchSeat { .. }
        | ClientMessage::StartBranch
        | ClientMessage::SelectMap { .. } => true,
    }
}
