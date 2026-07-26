use super::entities::validate_single_builders;
use super::CheckpointPayloadError;
use crate::game::entity::{BuildPhase, Entity, EntityKind, Order};

#[test]
fn rejects_multiple_active_builders_for_one_site() {
    let site = 7;
    let mut workers = [(), ()].map(|()| {
        let mut worker =
            Entity::new_unit(1, EntityKind::Worker, 32.0, 32.0).expect("worker should spawn");
        worker.set_order(Order::build(EntityKind::Depot, 1, 1));
        worker.mark_build_phase(BuildPhase::Constructing { site });
        worker
    });
    assert!(matches!(
        validate_single_builders(&workers),
        Err(CheckpointPayloadError::DuplicateId {
            field: "entities.order.buildSite",
            id: 7
        })
    ));
    workers[1].clear_active_order();
    assert!(validate_single_builders(&workers).is_ok());
}
