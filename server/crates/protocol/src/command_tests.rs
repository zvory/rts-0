use super::*;

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
