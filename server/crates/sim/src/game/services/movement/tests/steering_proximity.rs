use super::super::*;
use crate::game::entity::{EntityKind, EntityStore, WeaponSetup};
use crate::game::map::Map;

fn steering_with_blocker_offset(offset: (f32, f32)) -> (f32, f32) {
    let map = Map::generate(1, 0xC0FF_EE01);
    let mut entities = EntityStore::new();
    let (sx, sy) = map.tile_center(20, 20);
    let mover = entities
        .spawn_unit(1, EntityKind::Rifleman, sx, sy)
        .expect("mover spawn");
    let blocker = entities
        .spawn_unit(2, EntityKind::MachineGunner, sx + offset.0, sy + offset.1)
        .expect("blocker spawn");
    entities
        .get_mut(blocker)
        .expect("blocker")
        .set_weapon_setup(WeaponSetup::Deployed);
    let spatial = SpatialIndex::build(&entities, map.width, map.height);
    steering::local_steering_dir(&entities, &spatial, mover, sx, sy, (1.0, 0.0))
}

#[test]
fn distant_neighbor_exerts_less_steering_than_close_neighbor() {
    let close = steering_with_blocker_offset((24.0, 12.0));
    let distant = steering_with_blocker_offset((64.0, 32.0));
    assert!(
        close.1.abs() > distant.1.abs(),
        "closer traffic should exert more lateral steering: close={close:?}, distant={distant:?}"
    );
}
