import assert from "node:assert/strict";
import { ABILITY, KIND } from "../../client/src/protocol.js";
import { createRigRenderContext, sampleRigAnimation } from "../../client/src/renderer/rigs/animation.js";
import { compileSvgRig } from "../../client/src/renderer/rigs/svg_importer.js";
import { ROCKET_LAUNCHER_RIG_SVG } from "../../client/src/renderer/rigs/vehicle_svg.js";

export function assertRocketLauncherRackCooldownContract() {
  const compiled = compileSvgRig(ROCKET_LAUNCHER_RIG_SVG, { expectedKind: KIND.ROCKET_LAUNCHER });
  assert.equal(compiled.ok, true);
  const readyEntity = {
    id: 270,
    kind: KIND.ROCKET_LAUNCHER,
    owner: 1,
    facing: 0,
    abilities: [{ ability: ABILITY.BARRAGE, cooldownLeft: 0 }],
  };
  const context = (entity) => createRigRenderContext(entity, {
    now: 12_345,
    colorByOwner: new Map([[1, 0x0072b2]]),
  });
  const ready = sampleRigAnimation(compiled.definition, readyEntity, context(readyEntity));
  assert.equal(ready.parts["part.rocket.1"].visible, true);
  assert.equal(ready.parts["part.rocket.1"].tintSlot, "team-light-10");
  assert.equal(ready.parts["part.rocket.cooldown.1"].visible, false);

  const coolingEntity = {
    ...readyEntity,
    abilities: [{ ability: ABILITY.BARRAGE, cooldownLeft: 449 }],
  };
  const cooling = sampleRigAnimation(compiled.definition, coolingEntity, context(coolingEntity));
  assert.equal(cooling.parts["part.rocket.1"].visible, false);
  assert.equal(cooling.parts["part.rocket.cooldown.1"].visible, true);
  assert.equal(cooling.parts["part.rocket.cooldown.1"].paint.fill, "#24252a");
}
