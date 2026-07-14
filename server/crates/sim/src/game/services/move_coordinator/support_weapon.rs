use crate::config;
use crate::game::entity::{Entity, EntityKind, WeaponSetup};

pub(super) fn begin_weapon_teardown_for_movement(entity: &mut Entity) {
    let teardown_ticks = match entity.kind {
        EntityKind::MachineGunner => config::MACHINE_GUNNER_SETUP_TICKS,
        EntityKind::AntiTankGun => config::ANTI_TANK_GUN_SETUP_TICKS,
        EntityKind::Artillery => {
            entity.reset_artillery_accuracy();
            entity.reset_artillery_blanket_sequence();
            config::ARTILLERY_SETUP_TICKS
        }
        _ => return,
    };
    match entity.weapon_setup() {
        WeaponSetup::Packed | WeaponSetup::TearingDown { .. } => {}
        WeaponSetup::TearingDownToRedeploy { ticks } => {
            entity.set_pending_redeploy_facing(None);
            entity.set_weapon_setup(WeaponSetup::TearingDown { ticks });
        }
        WeaponSetup::SettingUp { .. } | WeaponSetup::Deployed => {
            entity.set_pending_redeploy_facing(None);
            entity.set_weapon_setup(WeaponSetup::TearingDown {
                ticks: teardown_ticks,
            });
        }
    }
}
