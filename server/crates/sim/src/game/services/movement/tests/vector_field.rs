use super::*;
use crate::game::ability::AbilityKind;
use crate::game::ability_runtime::{AbilityObjectPayload, AbilityWorldObjectKind, AbilityWorldObjectSpec};
fn field(center: (f32,f32), direction: [f32;2]) -> AbilityRuntime {
    let mut runtime = AbilityRuntime::new();
    runtime.spawn_world_object(AbilityWorldObjectSpec { owner:1,caster_id:999,
        ability:AbilityKind::EkatMagicAnchor, kind:AbilityWorldObjectKind::MagicAnchor,
        x:center.0,y:center.1,created_tick:0,expires_tick:100,
        payload:AbilityObjectPayload::VectorField {direction}, }).unwrap();
    runtime
}
#[test]
fn prototype_vector_field_rectangle_is_centered_rotated_and_expires() {
    let runtime = field((400.0,400.0), [0.0,1.0]);
    assert_eq!(runtime.vector_field_at(447.0,495.0,1),Some([0.0,1.0]));
    assert_eq!(runtime.vector_field_at(449.0,400.0,1),None);
    assert_eq!(runtime.vector_field_at(400.0,497.0,1),None);
    assert_eq!(runtime.vector_field_at(400.0,400.0,100),None);
}
#[test]
fn prototype_vector_field_moves_idle_tank_at_half_speed_and_also_moves_allies() {
    let map = flat_map(2);
    let center=map.tile_center(10,10);
    let players=[player_with_oil(1,0),player_with_oil(2,0)];
    for owner in [1,2] {
        for kind in [EntityKind::Rifleman,EntityKind::Tank] {
            let mut entities=EntityStore::new();
            let id=entities.spawn_unit(owner,kind,center.0,center.1).unwrap();
            let occ=Occupancy::build(&map,&entities);
            apply_magic_anchor_pull(&map,&mut entities,&players,&occ,1,&field(center,[0.0,1.0]));
            let unit=entities.get(id).unwrap();
            assert_eq!(unit.pos_x(),center.0);
            let base=config::unit_stats(EntityKind::Tank).unwrap().speed*0.5;
            let expected=if kind==EntityKind::Tank || !config::VECTOR_FIELD_WEIGHTED {base} else {base*12.0_f32.det_sqrt()};
            assert!((unit.pos_y()-center.1-expected).abs()<0.02,"{kind:?} owner={owner}");
            assert!(unit.facing().abs()<0.001,"field displacement does not rotate the body");
        }
    }
}
