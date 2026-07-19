use super::*;

#[test]
fn formation_move_uses_object_points_and_defaults_queue_false() {
    let command: Command = serde_json::from_str(
        r#"{"c":"formationMove","units":[3,4],"points":[{"x":10.0,"y":20.0},{"x":30.0,"y":40.0}]}"#,
    )
    .expect("formation move should deserialize");
    assert!(matches!(
        command,
        Command::FormationMove { units, points, queued: false }
            if units == vec![3, 4] && points.len() == 2
    ));
}

#[test]
fn command_messages_require_client_sequence_envelope() {
    let msg: ClientMessage = serde_json::from_str(
        r#"{"t":"command","clientSeq":7,"cmd":{"c":"move","units":[1,2],"x":10.0,"y":20.0}}"#,
    )
    .expect("sequenced command should deserialize");

    match msg {
        ClientMessage::Command { client_seq, cmd } => {
            assert_eq!(client_seq, 7);
            assert!(matches!(cmd, Command::Move { units, .. } if units == vec![1, 2]));
        }
        other => panic!("unexpected message: {other:?}"),
    }

    let missing_seq = serde_json::from_str::<ClientMessage>(
        r#"{"t":"command","cmd":{"c":"move","units":[1],"x":10.0,"y":20.0}}"#,
    );
    assert!(missing_seq.is_err());
}

#[test]
fn cancel_command_distinguishes_construction_from_legacy_production_scope() {
    let production: Command =
        serde_json::from_str(r#"{"c":"cancel","building":7}"#).expect("production cancel");
    assert!(matches!(
        production,
        Command::Cancel {
            building: 7,
            construction: false,
        }
    ));

    let construction: Command =
        serde_json::from_str(r#"{"c":"cancel","building":7,"construction":true}"#)
            .expect("construction cancel");
    assert!(matches!(
        construction,
        Command::Cancel {
            building: 7,
            construction: true,
        }
    ));
    assert_eq!(
        serde_json::to_string(&construction).expect("serialize construction cancel"),
        r#"{"c":"cancel","building":7,"construction":true}"#
    );
}
