use serde::{de::DeserializeOwned, Serialize};

use super::CheckpointPayloadError;

pub(super) fn serde_convert<T, U>(value: T) -> Result<U, CheckpointPayloadError>
where
    T: Serialize,
    U: DeserializeOwned,
{
    serde_json::from_value(
        serde_json::to_value(value)
            .map_err(|err| CheckpointPayloadError::MalformedJson(err.to_string()))?,
    )
    .map_err(|err| CheckpointPayloadError::MalformedJson(err.to_string()))
}
