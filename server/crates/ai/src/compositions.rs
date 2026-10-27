//! Weighted, once-per-match choices among concrete AI behaviors.
use rand::{rngs::SmallRng, Rng, SeedableRng};
use serde::Serialize;

use crate::ai_core::profiles::{JEFFS_AI_ID, PRESSURE_AI_ID};
use crate::rush::RUSH_AI_ID;

pub(crate) const AI_2_3_ID: &str = "ai_2_3";

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WeightedProfile {
    pub(crate) profile_id: &'static str,
    pub(crate) weight: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(crate) struct AiComposition {
    pub(crate) id: &'static str,
    pub(crate) label: &'static str,
    pub(crate) members: &'static [WeightedProfile],
}

pub(crate) const COMPOSITIONS: &[AiComposition] = &[AiComposition {
    id: AI_2_3_ID,
    label: "AI 2.3",
    members: &[
        WeightedProfile {
            profile_id: RUSH_AI_ID,
            weight: 1,
        },
        WeightedProfile {
            profile_id: PRESSURE_AI_ID,
            weight: 1,
        },
        WeightedProfile {
            profile_id: JEFFS_AI_ID,
            weight: 1,
        },
    ],
}];

pub(crate) fn composition_by_id(id: &str) -> Option<&'static AiComposition> {
    COMPOSITIONS.iter().find(|composition| composition.id == id)
}

impl AiComposition {
    fn total_weight(&self) -> Option<u64> {
        self.members
            .iter()
            .try_fold(0u64, |sum, member| {
                sum.checked_add(u64::from(member.weight))
            })
            .filter(|sum| *sum > 0)
    }

    fn at_ticket(&self, mut ticket: u64) -> Option<&'static str> {
        for member in self.members {
            let weight = u64::from(member.weight);
            if ticket < weight {
                return Some(member.profile_id);
            }
            ticket = ticket.checked_sub(weight)?;
        }
        None
    }

    pub(crate) fn select(&self, match_seed: u32, player: u32) -> Option<&'static str> {
        // Independent from simulation RNG and from controller iteration order.
        let seed = ((u64::from(match_seed) << 32) | u64::from(player)) ^ 0xa122_5e1e_c710_0001;
        let mut rng = SmallRng::seed_from_u64(seed);
        self.at_ticket(rng.gen_range(0..self.total_weight()?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai_core::profiles::profile_by_id;
    use std::collections::BTreeSet;

    #[test]
    fn registry_contains_valid_concrete_choices() {
        let mut ids = BTreeSet::new();
        for composition in COMPOSITIONS {
            assert!(ids.insert(composition.id));
            assert!(!composition.members.is_empty());
            let mut members = BTreeSet::new();
            for member in composition.members {
                assert!(members.insert(member.profile_id));
                assert!(member.weight > 0);
                assert!(
                    member.profile_id == RUSH_AI_ID || profile_by_id(member.profile_id).is_some()
                );
            }
        }
    }

    #[test]
    fn two_point_three_has_equal_rush_pressure_and_jeff_weights() {
        let composition = composition_by_id(AI_2_3_ID).unwrap();
        assert_eq!(composition.total_weight(), Some(3));
        assert_eq!(composition.at_ticket(0), Some(RUSH_AI_ID));
        assert_eq!(composition.at_ticket(1), Some(PRESSURE_AI_ID));
        assert_eq!(composition.at_ticket(2), Some(JEFFS_AI_ID));
        assert_eq!(composition.at_ticket(3), None);
        let selected = (0..128)
            .flat_map(|seed| {
                [1, 2].map(|player| {
                    let selection = composition.select(seed, player).unwrap();
                    assert_eq!(composition.select(seed, player), Some(selection));
                    selection
                })
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(
            selected,
            BTreeSet::from([RUSH_AI_ID, PRESSURE_AI_ID, JEFFS_AI_ID])
        );
    }

    #[test]
    fn more_members_and_unequal_weights_use_the_same_selector() {
        let composition = AiComposition {
            id: "test",
            label: "test",
            members: &[
                WeightedProfile {
                    profile_id: "a",
                    weight: 1,
                },
                WeightedProfile {
                    profile_id: "b",
                    weight: 3,
                },
                WeightedProfile {
                    profile_id: "c",
                    weight: 2,
                },
            ],
        };
        assert_eq!(
            (0..6)
                .map(|ticket| composition.at_ticket(ticket).unwrap())
                .collect::<Vec<_>>(),
            vec!["a", "b", "b", "b", "c", "c"]
        );
    }
}
