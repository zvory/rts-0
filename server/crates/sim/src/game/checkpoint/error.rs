#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::game) enum CheckpointPayloadError {
    PayloadTooLarge {
        bytes: usize,
        max: usize,
    },
    MalformedJson(String),
    UnsupportedSchema {
        found: String,
    },
    UnsupportedVersion {
        found: u32,
    },
    UnsupportedRequiredFeature {
        feature: String,
    },
    IncompatibleRngAlgorithm {
        found: String,
    },
    MapBindingMismatch {
        field: &'static str,
    },
    CountCapExceeded {
        field: &'static str,
        count: usize,
        max: usize,
    },
    DuplicateId {
        field: &'static str,
        id: u32,
    },
    InvalidReference {
        field: &'static str,
        id: u32,
    },
    InvalidValue {
        field: &'static str,
    },
}

impl std::fmt::Display for CheckpointPayloadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CheckpointPayloadError::PayloadTooLarge { bytes, max } => {
                write!(f, "checkpoint payload is {bytes} bytes; max is {max}")
            }
            CheckpointPayloadError::MalformedJson(err) => {
                write!(f, "checkpoint payload is not valid JSON: {err}")
            }
            CheckpointPayloadError::UnsupportedSchema { found } => {
                write!(f, "unsupported checkpoint schema {found:?}")
            }
            CheckpointPayloadError::UnsupportedVersion { found } => {
                write!(f, "unsupported checkpoint version {found}")
            }
            CheckpointPayloadError::UnsupportedRequiredFeature { feature } => {
                write!(f, "unsupported checkpoint required feature {feature:?}")
            }
            CheckpointPayloadError::IncompatibleRngAlgorithm { found } => {
                write!(f, "incompatible checkpoint RNG algorithm {found:?}")
            }
            CheckpointPayloadError::MapBindingMismatch { field } => {
                write!(f, "checkpoint map binding mismatch for {field}")
            }
            CheckpointPayloadError::CountCapExceeded { field, count, max } => {
                write!(f, "checkpoint {field} count {count} exceeds cap {max}")
            }
            CheckpointPayloadError::DuplicateId { field, id } => {
                write!(f, "checkpoint duplicate {field} id {id}")
            }
            CheckpointPayloadError::InvalidReference { field, id } => {
                write!(f, "checkpoint invalid {field} reference {id}")
            }
            CheckpointPayloadError::InvalidValue { field } => {
                write!(f, "checkpoint invalid value for {field}")
            }
        }
    }
}

impl std::error::Error for CheckpointPayloadError {}
