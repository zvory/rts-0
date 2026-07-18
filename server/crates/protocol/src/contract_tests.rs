use super::*;

#[test]
fn protocol_contract_metadata_matches_public_constants() {
    let contract = serde_json::to_value(protocol_contract()).unwrap();

    assert_eq!(contract["schemaVersion"], serde_json::json!(1));
    assert_eq!(
        contract["compactSnapshotVersion"],
        serde_json::json!(COMPACT_SNAPSHOT_VERSION)
    );
    assert_eq!(
        contract["predictionProtocolVersion"],
        serde_json::json!(PREDICTION_PROTOCOL_VERSION)
    );
    assert_eq!(
        contract["unknownCodeSentinel"],
        serde_json::json!(COMPACT_UNKNOWN_CODE)
    );
    assert_eq!(
        contract["snapshotCodecs"]["defaultCodec"],
        serde_json::json!(SNAPSHOT_CODEC_MESSAGEPACK_COMPACT)
    );
    assert_eq!(
        contract["compactCodes"]["kind"][kinds::WORKER],
        serde_json::json!(kind_code(kinds::WORKER))
    );
    assert_eq!(
        contract["compactCodes"]["ability"][abilities::EKAT_MAGIC_ANCHOR],
        serde_json::json!(ability_code(abilities::EKAT_MAGIC_ANCHOR))
    );
    let entity_schema = contract["compactSlotSchemas"]["entity"].as_array().unwrap();
    assert_eq!(entity_schema[39]["name"], "extractorActive");
}
