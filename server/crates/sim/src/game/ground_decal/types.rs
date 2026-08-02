use serde::{de::Error as _, Deserialize, Deserializer, Serialize, Serializer};

use crate::config;
use crate::game::entity::EntityKind;
use crate::protocol;
use crate::rules::{
    artillery_ground_decal_source_kind, death_ground_decal_class, mortar_ground_decal_source_kind,
};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub(super) enum GroundDecalClass {
    #[serde(rename = "infantry")]
    Infantry,
    #[serde(rename = "scorch")]
    Scorch,
    #[serde(rename = "buildingScorch")]
    BuildingScorch,
    #[serde(rename = "mortarBlast")]
    MortarBlast,
    #[serde(rename = "artilleryBlast")]
    ArtilleryBlast,
}

pub(super) fn valid_class_and_source(decal: &super::GroundDecal) -> bool {
    match decal.decal_class {
        GroundDecalClass::MortarBlast => {
            decal.source_kind == mortar_ground_decal_source_kind()
                && decal
                    .radius_tiles
                    .is_some_and(|radius| radius == config::MORTAR_OUTER_RADIUS_TILES)
                && decal.owner == 0
        }
        GroundDecalClass::ArtilleryBlast => {
            decal.source_kind == artillery_ground_decal_source_kind()
                && decal
                    .radius_tiles
                    .is_some_and(|radius| radius == config::ARTILLERY_OUTER_RADIUS_TILES)
                && decal.owner == 0
        }
        class => {
            decal.radius_tiles.is_none()
                && GroundDecalClass::from_death_kind(decal.source_kind) == Some(class)
        }
    }
}

impl GroundDecalClass {
    pub(super) fn from_death_kind(kind: EntityKind) -> Option<Self> {
        match death_ground_decal_class(kind)? {
            "infantry" => Some(Self::Infantry),
            "scorch" => Some(Self::Scorch),
            "buildingScorch" => Some(Self::BuildingScorch),
            _ => None,
        }
    }

    pub(super) fn wire_name(self) -> &'static str {
        match self {
            Self::Infantry => "infantry",
            Self::Scorch => "scorch",
            Self::BuildingScorch => "buildingScorch",
            Self::MortarBlast => "mortarBlast",
            Self::ArtilleryBlast => "artilleryBlast",
        }
    }
}

pub(super) fn serialize_source_kind<S>(kind: &EntityKind, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(protocol::kind_to_wire(*kind))
}

pub(super) fn deserialize_source_kind<'de, D>(deserializer: D) -> Result<EntityKind, D::Error>
where
    D: Deserializer<'de>,
{
    let wire = String::deserialize(deserializer)?;
    protocol::kind_from_wire(&wire)
        .ok_or_else(|| D::Error::custom(format!("unknown decal source kind {wire}")))
}
