use rts_rules::defs::{self, TechRequirement};
use rts_rules::faction::{catalog_for, FactionCatalog, UpgradeKind, CURRENT_CATALOG};
use rts_rules::EntityKind;

use super::AiFrame;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AiCost {
    pub steel: u32,
    pub oil: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AiFootprint {
    pub width_tiles: u32,
    pub height_tiles: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AiPrerequisites {
    All(Vec<EntityKind>),
    Any(Vec<EntityKind>),
}

impl AiPrerequisites {
    pub fn is_met(&self, owned_complete_buildings: &[EntityKind]) -> bool {
        match self {
            Self::All(required) => required
                .iter()
                .all(|kind| owned_complete_buildings.contains(kind)),
            Self::Any(required) => required
                .iter()
                .any(|kind| owned_complete_buildings.contains(kind)),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AiEntityRule {
    pub kind: EntityKind,
    pub cost: AiCost,
    pub supply: u32,
    pub production_ticks: u32,
    pub maximum_health: u32,
    pub footprint: Option<AiFootprint>,
    pub builders: Vec<EntityKind>,
    pub producers: Vec<EntityKind>,
    pub prerequisites: AiPrerequisites,
    pub can_gather: bool,
}

/// A faction-bound view over the authoritative `rts-rules` catalog and definitions.
///
/// This type owns no rule data. Unknown factions and kinds outside the selected faction return
/// `None`/`false` rather than silently falling back to another catalog.
#[derive(Clone, Copy, Debug)]
pub struct AiRulebook {
    catalog: FactionCatalog,
}

impl AiRulebook {
    pub fn new(faction_id: &str) -> Option<Self> {
        catalog_for(faction_id).map(|catalog| Self { catalog })
    }

    pub fn for_frame(frame: &AiFrame) -> Option<Self> {
        Self::new(frame.faction_id())
    }

    pub(crate) fn compatibility_default() -> Self {
        Self {
            catalog: CURRENT_CATALOG,
        }
    }

    pub fn faction_id(self) -> &'static str {
        self.catalog.id
    }

    /// Entity kinds in authoritative faction-catalog order: units first, then buildings.
    pub fn entities(self) -> impl Iterator<Item = EntityKind> {
        self.catalog
            .units
            .iter()
            .chain(self.catalog.buildings)
            .copied()
    }

    pub fn is_available(self, kind: EntityKind) -> bool {
        self.catalog.allows_unit(kind) || self.catalog.allows_building(kind)
    }

    pub fn entity(self, kind: EntityKind) -> Option<AiEntityRule> {
        if !self.is_available(kind) {
            return None;
        }

        if let Some(definition) = defs::unit_def(kind) {
            let producers = definition
                .trained_at
                .filter(|producer| self.catalog.trainable_units(*producer).contains(&kind))
                .into_iter()
                .collect();
            return Some(AiEntityRule {
                kind,
                cost: AiCost {
                    steel: definition.stats.cost_steel,
                    oil: definition.stats.cost_oil,
                },
                supply: definition.stats.supply,
                production_ticks: definition.stats.build_ticks,
                maximum_health: definition.stats.hp,
                footprint: None,
                builders: Vec::new(),
                producers,
                prerequisites: prerequisites(definition.train_requirement),
                can_gather: self.catalog.can_gather(kind),
            });
        }

        let definition = defs::building_def(kind)?;
        Some(AiEntityRule {
            kind,
            cost: AiCost {
                steel: definition.stats.cost_steel,
                oil: definition.stats.cost_oil,
            },
            supply: 0,
            production_ticks: definition.stats.build_ticks,
            maximum_health: definition.stats.hp,
            footprint: Some(AiFootprint {
                width_tiles: definition.stats.foot_w,
                height_tiles: definition.stats.foot_h,
            }),
            builders: self
                .catalog
                .builders
                .iter()
                .copied()
                .filter(|builder| self.catalog.can_build(*builder, kind))
                .collect(),
            producers: Vec::new(),
            prerequisites: AiPrerequisites::All(definition.build_requires.to_vec()),
            can_gather: false,
        })
    }

    pub fn cost(self, kind: EntityKind) -> Option<AiCost> {
        self.entity(kind).map(|rule| rule.cost)
    }

    pub fn trainable_units(self, producer: EntityKind) -> Vec<EntityKind> {
        self.catalog.trainable_units(producer)
    }

    pub fn can_train(self, producer: EntityKind, unit: EntityKind) -> bool {
        self.catalog.can_act_as_production_anchor(producer)
            && self.catalog.allows_unit(unit)
            && defs::building_def(producer)
                .is_some_and(|definition| definition.trains.contains(&unit))
    }

    pub fn researchable_upgrades(self, producer: EntityKind) -> Vec<UpgradeKind> {
        self.catalog.researchable_upgrade_kinds(producer).collect()
    }

    pub fn can_build(self, builder: EntityKind, building: EntityKind) -> bool {
        self.catalog.can_build(builder, building)
    }

    pub fn can_gather(self, kind: EntityKind) -> bool {
        self.catalog.can_gather(kind)
    }

    pub fn prerequisites_met(
        self,
        kind: EntityKind,
        owned_complete_buildings: &[EntityKind],
    ) -> bool {
        self.entity(kind)
            .is_some_and(|rule| rule.prerequisites.is_met(owned_complete_buildings))
    }
}

fn prerequisites(requirement: TechRequirement) -> AiPrerequisites {
    match requirement {
        TechRequirement::All(required) => AiPrerequisites::All(required.to_vec()),
        TechRequirement::Any(required) => AiPrerequisites::Any(required.to_vec()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rts_rules::faction::{CURRENT_CATALOG, DEFAULT_FACTION_ID};

    #[test]
    fn answers_delegate_to_authoritative_catalog_and_definitions_in_order() {
        let rules = AiRulebook::new(DEFAULT_FACTION_ID).unwrap();
        let expected = CURRENT_CATALOG
            .units
            .iter()
            .chain(CURRENT_CATALOG.buildings)
            .copied()
            .collect::<Vec<_>>();
        assert_eq!(rules.entities().collect::<Vec<_>>(), expected);

        for kind in expected {
            let answer = rules.entity(kind).unwrap();
            assert_eq!(answer.cost.steel, rts_rules::economy::cost(kind).0);
            assert_eq!(answer.cost.oil, rts_rules::economy::cost(kind).1);
            assert_eq!(answer.supply, rts_rules::economy::supply_cost(kind));
            if let Some(stats) = rts_rules::balance::unit_stats(kind) {
                assert_eq!(answer.maximum_health, stats.hp);
                assert_eq!(answer.production_ticks, stats.build_ticks);
            } else {
                let stats = rts_rules::balance::building_stats(kind).unwrap();
                assert_eq!(answer.maximum_health, stats.hp);
                assert_eq!(answer.production_ticks, stats.build_ticks);
                assert_eq!(
                    answer.footprint,
                    Some(AiFootprint {
                        width_tiles: stats.foot_w,
                        height_tiles: stats.foot_h,
                    })
                );
            }
        }
    }

    #[test]
    fn faction_filtering_and_relationships_remain_authoritative() {
        let rules = AiRulebook::new(DEFAULT_FACTION_ID).unwrap();
        assert!(rules.can_gather(EntityKind::Worker));
        assert!(!rules.is_available(EntityKind::Golem));
        assert_eq!(
            rules.entity(EntityKind::Tank).unwrap().producers,
            vec![EntityKind::Factory]
        );
        assert_eq!(
            rules.trainable_units(EntityKind::Barracks),
            rts_rules::economy::trainable_units_for_faction(
                DEFAULT_FACTION_ID,
                EntityKind::Barracks
            )
        );
    }
}
