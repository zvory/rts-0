use super::*;
use rts_rules::math::GameplayMath as _;

#[allow(clippy::too_many_arguments)]
pub(super) fn place(
    ctx: &mut CommandExecutionContext<'_, '_>,
    players: &mut [PlayerState],
    player: u32,
    faction_id: &str,
    units: Vec<u32>,
    center: (f32, f32),
    direction: (f32, f32),
    admission: CommandAdmissionPolicy,
) {
    let length = direction.0.det_hypot(direction.1);
    if !center.0.is_finite() || !center.1.is_finite() || !length.is_finite() || length < 0.001 {
        return;
    }
    let Some(units) = validate_command_units(ctx.entities, ctx.events, player, units, admission)
    else {
        return;
    };
    for caster in units {
        if launch_world_ability(
            ctx.map,
            ctx.entities,
            players,
            &ctx.teams,
            ctx.smokes,
            ctx.ability_runtime,
            ctx.mortar_shells,
            ctx.events,
            player,
            faction_id,
            caster,
            AbilityKind::VectorField,
            center.0,
            center.1,
            ctx.tick,
            true,
            true,
        ) {
            ctx.ability_runtime.configure_vector_field(
                player,
                caster,
                [direction.0 / length, direction.1 / length],
            );
            break;
        }
    }
}
