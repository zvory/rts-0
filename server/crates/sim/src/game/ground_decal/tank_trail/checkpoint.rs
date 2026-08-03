use std::collections::BTreeMap;

use base64::{engine::general_purpose::STANDARD_NO_PAD, Engine as _};
use serde::{de::Error as _, Deserialize, Deserializer, Serialize, Serializer};

use super::{
    ActiveTankTrail, FinalizedTankTrail, TankTrailPose, TankTrailSpatialIndex, TankTrailStore,
    TrailBounds, MAX_ACTIVE_POSES,
};

impl Serialize for TankTrailStore {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let finalized = self
            .finalized
            .iter()
            .map(|trail| {
                (
                    trail.created_revision,
                    trail.owner,
                    encode_poses(&trail.poses),
                )
            })
            .collect::<Vec<_>>();
        let active = self
            .active_by_tank
            .iter()
            .map(|(tank_id, trail)| {
                (
                    *tank_id,
                    trail.owner,
                    encode_poses(&trail.poses),
                    trail.last_observed,
                    trail.last_motion_tick,
                )
            })
            .collect::<Vec<_>>();
        (finalized, active).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for TankTrailStore {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        type FinalizedRow = (u32, u32, String);
        type ActiveRow = (u32, u32, String, TankTrailPose, u32);
        let (finalized_rows, active_rows): (Vec<FinalizedRow>, Vec<ActiveRow>) =
            Deserialize::deserialize(deserializer)?;
        let mut finalized = Vec::with_capacity(finalized_rows.len());
        for (index, (created_revision, owner, encoded)) in finalized_rows.into_iter().enumerate() {
            let poses = decode_poses::<D::Error>(&encoded)?;
            if poses.len() < 2 || poses.len() > MAX_ACTIVE_POSES {
                return Err(D::Error::custom("invalid finalized tank trail pose count"));
            }
            let id = u32::try_from(index)
                .ok()
                .and_then(|value| value.checked_add(1))
                .ok_or_else(|| D::Error::custom("too many finalized tank trails"))?;
            let bounds = TrailBounds::from_poses(&poses)
                .ok_or_else(|| D::Error::custom("invalid finalized tank trail bounds"))?;
            finalized.push(FinalizedTankTrail {
                id,
                owner,
                poses,
                bounds,
                created_revision,
            });
        }
        let next_id = u32::try_from(finalized.len())
            .ok()
            .and_then(|value| value.checked_add(1))
            .ok_or_else(|| D::Error::custom("too many finalized tank trails"))?;
        let mut active_by_tank = BTreeMap::new();
        for (tank_id, owner, encoded, last_observed, last_motion_tick) in active_rows {
            let poses = decode_poses::<D::Error>(&encoded)?;
            if poses.is_empty() || poses.len() > MAX_ACTIVE_POSES {
                return Err(D::Error::custom("invalid active tank trail pose count"));
            }
            let previous = active_by_tank.insert(
                tank_id,
                ActiveTankTrail {
                    owner,
                    poses,
                    last_observed,
                    last_motion_tick,
                },
            );
            if previous.is_some() {
                return Err(D::Error::custom("duplicate active tank trail"));
            }
        }
        Ok(TankTrailStore {
            next_id,
            finalized,
            active_by_tank,
            spatial_index: TankTrailSpatialIndex::default(),
        })
    }
}

pub(super) fn encode_poses(poses: &[TankTrailPose]) -> String {
    let mut bytes = Vec::with_capacity(poses.len() * 5);
    for pose in poses {
        bytes.extend_from_slice(&pose.0 .0.to_le_bytes());
        bytes.extend_from_slice(&pose.0 .1.to_le_bytes());
        bytes.push(pose.0 .2 as u8);
    }
    STANDARD_NO_PAD.encode(bytes)
}

pub(super) fn decode_poses<E: serde::de::Error>(encoded: &str) -> Result<Vec<TankTrailPose>, E> {
    let bytes = STANDARD_NO_PAD
        .decode(encoded)
        .map_err(|_| E::custom("invalid tank trail pose encoding"))?;
    if !bytes.len().is_multiple_of(5) {
        return Err(E::custom("invalid tank trail pose byte length"));
    }
    bytes
        .chunks_exact(5)
        .map(|bytes| {
            let heading = bytes[4] as i8;
            if heading == i8::MIN {
                return Err(E::custom("invalid tank trail heading"));
            }
            Ok(TankTrailPose((
                u16::from_le_bytes([bytes[0], bytes[1]]),
                u16::from_le_bytes([bytes[2], bytes[3]]),
                heading,
            )))
        })
        .collect()
}
