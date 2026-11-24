use super::super::standability::{footing_profile, FootingProfile};
use super::*;

#[test]
fn cultivator_combat_footing_resists_incoming_cultivators() {
    for kind in [
        EntityKind::Warrior,
        EntityKind::LightnessAdept,
        EntityKind::WarAdept,
        EntityKind::MagicAdept,
    ] {
        for master in [false, true] {
            for blocker_first in [false, true] {
                let map = flat_map(1);
                let mut entities = EntityStore::new();
                let (x, y) = map.tile_center(20, 20);
                let (blocker, mover) = if blocker_first {
                    let blocker = entities.spawn_unit(1, kind, x + 20.0, y).unwrap();
                    (blocker, entities.spawn_unit(1, kind, x, y).unwrap())
                } else {
                    let mover = entities.spawn_unit(1, kind, x, y).unwrap();
                    (entities.spawn_unit(1, kind, x + 20.0, y).unwrap(), mover)
                };
                if master && kind != EntityKind::Warrior {
                    let unit = entities.get_mut(blocker).unwrap();
                    unit.insight = config::DISCIPLE_EVOLVED_INSIGHT_CAP;
                    unit.advance_dao_master();
                    assert!(unit.dao_master);
                }
                entities
                    .get_mut(blocker)
                    .unwrap()
                    .set_target_id(Some(mover));
                mark_moving(&mut entities, mover, (x + 200.0, y));
                entities.get_mut(mover).unwrap().set_facing(0.0);
                assert_eq!(
                    footing_profile(entities.get(blocker).unwrap()),
                    FootingProfile::Rooted
                );
                let before = pos(&entities, blocker);
                let mover_before = pos(&entities, mover);
                let occ = Occupancy::build(&map, &entities);
                let spatial = SpatialIndex::build(&entities, map.width, map.height);
                resolve_collisions(&mut entities, &spatial, &map, &occ);
                let displacement = moved_distance(before, pos(&entities, blocker));
                let mover_displacement = moved_distance(mover_before, pos(&entities, mover));
                assert!(
                    mover_displacement > displacement * 50.0,
                    "{kind:?} master={master}: blocker moved {displacement}, mover moved {mover_displacement}"
                );
                assert_eq!(
                    pos(&entities, blocker).1,
                    y,
                    "combatant must not receive the lateral traffic nudge"
                );
                assert!(body_overlap_depth(&entities, blocker, mover) <= COLLISION_EPS_PX);
                entities
                    .get_mut(blocker)
                    .unwrap()
                    .set_path(vec![(x + 200.0, y)]);
                assert_eq!(
                    footing_profile(entities.get(blocker).unwrap()),
                    FootingProfile::Soft
                );
                entities.get_mut(blocker).unwrap().clear_path();
                entities.get_mut(blocker).unwrap().set_target_id(None);
                assert_eq!(
                    footing_profile(entities.get(blocker).unwrap()),
                    FootingProfile::Soft
                );
            }
        }
    }
}
