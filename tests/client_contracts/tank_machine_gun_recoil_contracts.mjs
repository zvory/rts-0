import assert from "node:assert/strict";
import { KIND, WEAPON_KIND } from "../../client/src/protocol.js";
import { createLiveRigDefinitions } from "../../client/src/renderer/rigs/live_routing.js";
import { createRigRenderContext, sampleRigAnimation, transformedRigAnchorPoint } from "../../client/src/renderer/rigs/animation.js";

const fixedNow = 1000;
{
  const definition = createLiveRigDefinitions().get(KIND.TANK);
  for (const commanderFacing of [0, Math.PI / 2, Math.PI, -Math.PI / 2]) {
    const entity = { id: 8, kind: KIND.TANK, owner: 1, x: 100, y: 100,
      facing: 0, weaponFacing: Math.PI / 4, commanderFacing };
    const idleContext = createRigRenderContext(entity, { now: fixedNow });
    const firingContext = createRigRenderContext(entity, { now: fixedNow,
      state: { weaponRecoil: (id, kind, now, weaponKind) => weaponKind === WEAPON_KIND.TANK_COAX ? 1 : 0 } });
    const idle = sampleRigAnimation(definition, entity, idleContext);
    const firing = sampleRigAnimation(definition, entity, firingContext);
    const scout = createRigRenderContext({ ...entity, kind: KIND.SCOUT_CAR, recoilProgress: 1 });
    const recoilPx = scout.recoilPx;
    for (const partId of ["part.hull", "part.turret", "part.barrel", "part.tank.flashCone"]) {
      assert.deepEqual(firing.parts[partId], idle.parts[partId], `${partId} is unaffected by machine gun fire`);
    }
    const muzzle = transformedRigAnchorPoint(definition, entity, "coaxMuzzle", idleContext);
    const firingMuzzle = transformedRigAnchorPoint(definition, entity, "coaxMuzzle", firingContext);
    assert.ok(Math.abs(firingMuzzle.x - muzzle.x + Math.cos(commanderFacing) * recoilPx) < 0.0001);
    assert.ok(Math.abs(firingMuzzle.y - muzzle.y + Math.sin(commanderFacing) * recoilPx) < 0.0001);
  }

}
