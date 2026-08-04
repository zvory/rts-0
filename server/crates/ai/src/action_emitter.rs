use crate::sdk::actions::AiActionRequest;
use rts_sim::game::command::SimCommand;

pub(crate) fn emit_request(request: AiActionRequest) -> SimCommand {
    match request {
        AiActionRequest::Move {
            units,
            x,
            y,
            queued,
        } => SimCommand::Move {
            units,
            x,
            y,
            queued,
        },
        AiActionRequest::AttackMove {
            units,
            x,
            y,
            queued,
        } => SimCommand::AttackMove {
            units,
            x,
            y,
            queued,
        },
        AiActionRequest::Attack {
            units,
            target,
            queued,
        } => SimCommand::Attack {
            units,
            target,
            queued,
        },
        AiActionRequest::Gather {
            units,
            node,
            queued,
        } => SimCommand::Gather {
            units,
            node,
            queued,
        },
        AiActionRequest::Build {
            units,
            building,
            tile_x,
            tile_y,
            queued,
        } => SimCommand::Build {
            units,
            building,
            tile_x,
            tile_y,
            queued,
        },
        AiActionRequest::Train { building, unit } => SimCommand::Train { building, unit },
        AiActionRequest::AdjustProductionRepeat {
            buildings,
            unit,
            delta,
        } => SimCommand::AdjustProductionRepeat {
            buildings,
            unit,
            delta,
        },
        AiActionRequest::Research { building, upgrade } => {
            SimCommand::Research { building, upgrade }
        }
        AiActionRequest::HoldPosition { units, queued } => {
            SimCommand::HoldPosition { units, queued }
        }
        AiActionRequest::SetupAntiTankGuns {
            units,
            x,
            y,
            queued,
        } => SimCommand::SetupAntiTankGuns {
            units,
            x,
            y,
            queued,
        },
    }
}
