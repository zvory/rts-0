use super::*;

#[test]
fn set_name_deserializes() {
    let msg: ClientMessage = serde_json::from_str(r#"{"t":"setName","name":"Renamed"}"#).unwrap();
    assert!(matches!(msg, ClientMessage::SetName { name } if name == "Renamed"));
}

#[test]
fn match_load_ready_deserializes_countdown_generation() {
    let msg: ClientMessage =
        serde_json::from_str(r#"{"t":"matchLoadReady","countdownId":17}"#).unwrap();
    assert!(matches!(
        msg,
        ClientMessage::MatchLoadReady { countdown_id: 17 }
    ));
}

#[test]
fn seek_replay_to_deserializes_absolute_tick() {
    let msg: ClientMessage = serde_json::from_str(r#"{"t":"seekRoomTimeTo","tick":4100}"#)
        .expect("seekRoomTimeTo should deserialize");

    match msg {
        ClientMessage::SeekRoomTimeTo { tick } => assert_eq!(tick, 4_100),
        other => panic!("expected seekRoomTimeTo, got {other:?}"),
    }
}

#[test]
fn set_vision_selection_deserializes() {
    let msg: ClientMessage = serde_json::from_str(
        r#"{"t":"setVisionSelection","selection":{"mode":"player","playerId":7}}"#,
    )
    .expect("setVisionSelection should deserialize");

    match msg {
        ClientMessage::SetVisionSelection {
            selection: VisionSelectionRequest::Player { player_id },
        } => assert_eq!(player_id, 7),
        other => panic!("expected setVisionSelection, got {other:?}"),
    }
}

#[test]
fn request_branch_from_tick_deserializes() {
    let msg: ClientMessage = serde_json::from_str(r#"{"t":"requestBranchFromTick"}"#)
        .expect("requestBranchFromTick should deserialize");

    assert!(matches!(msg, ClientMessage::RequestBranchFromTick));
}

#[test]
fn branch_staging_client_messages_deserialize() {
    let claim: ClientMessage = serde_json::from_str(r#"{"t":"claimBranchSeat","playerId":7}"#)
        .expect("claimBranchSeat should deserialize");
    let release: ClientMessage = serde_json::from_str(r#"{"t":"releaseBranchSeat","playerId":7}"#)
        .expect("releaseBranchSeat should deserialize");
    let start: ClientMessage =
        serde_json::from_str(r#"{"t":"startBranch"}"#).expect("startBranch should deserialize");

    assert!(matches!(
        claim,
        ClientMessage::ClaimBranchSeat { player_id: 7 }
    ));
    assert!(matches!(
        release,
        ClientMessage::ReleaseBranchSeat { player_id: 7 }
    ));
    assert!(matches!(start, ClientMessage::StartBranch));
}
