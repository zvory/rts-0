import assert from "node:assert/strict";
import fs from "node:fs";
import { ANTI_TANK_GUN_SETUP_TICKS, ANTI_TANK_GUN_TEARDOWN_TICKS, TICK_HZ } from "../../client/src/config.js";
import { supportWeaponSetupVisual } from "../../client/src/renderer/support_weapon_setup_visual.js";
import { createMinimapAnimation } from "../../client/src/renderer/rigs/minimap_animation.js";

const near = (a, b, message) => assert.ok(Math.abs(a - b) < 1e-9, `${message}: ${a} vs ${b}`);

export function assertAntiTankTrailContracts(renderer, original) {
  // Keep presentation timing tied to the authoritative mirror, including the
  // shorter teardown; importing a renderer must never change gameplay values.
  const rust = fs.readFileSync(new URL("../../server/crates/rules/src/balance/support_weapons.rs", import.meta.url), "utf8");
  for (const [name, ticks] of Object.entries({ ANTI_TANK_GUN_SETUP_TICKS, ANTI_TANK_GUN_TEARDOWN_TICKS })) {
    assert.match(rust, new RegExp(`pub const ${name}: u16 = ${ticks};`));
  }
  const records = new Map(), animation = createMinimapAnimation();
  const entity = { ...original, setupState: "packed", state: "move", facing: 0.7, weaponFacing: 1.2 };
  const state = { playerId: 1, weaponRecoil: () => 0 };
  const sample = (now) => {
    const visual = supportWeaponSetupVisual(entity, records, now);
    renderer._deployedWeaponSetupVisual = () => visual;
    renderer._drawUnit(entity, new Map([[1, 0x336699]]), state);
    animation.beginFrame([entity], state, now);
    const layers = animation.layers(entity);
    const unit = renderer._liveRigPools.liveUnitRigs.get(entity.id);
    assert.equal(layers.length, 4, "one opaque carriage/barrel and two attached trails at every stage");
    assert.ok(layers.every(layer => layer.alpha === 1));
    assert.ok(layers.every(layer => !layer.part.endsWith(".packed")));
    for (const layer of layers) {
      const display = unit.parts.get(layer.part.slice("sprite:".length)).display;
      near(display.rotation, layer.rotation, "minimap and world rotation");
      // Minimap icons use a shared 128-unit origin and 1.8 size compensation.
      const px = (display.pivotX - unit.parts.get(layer.part.slice(7)).sprite.frame.originX) * display.scaleX;
      const py = (display.pivotY - unit.parts.get(layer.part.slice(7)).sprite.frame.originY) * display.scaleY;
      const c = Math.cos(display.rotation), s = Math.sin(display.rotation);
      const scale = 1.8 / (128 * 1.05);
      near(layer.x, (display.x - px * c + py * s) * scale, "minimap and world hinge x");
      near(layer.y, (display.y - px * s - py * c) * scale, "minimap and world hinge y");
    }
    return { visual, layers, left: layers.find(layer => layer.part.includes("leftTrail")) };
  };
  const packed = sample(0);
  entity.setupState = "setting_up"; entity.state = "idle"; entity.weaponFacing = entity.facing;
  sample(100);
  const duration = ANTI_TANK_GUN_SETUP_TICKS / TICK_HZ * 1000;
  const pause = sample(100 + duration * 0.82);
  const held = sample(100 + duration * 0.97);
  near(pause.left.rotation, held.left.rotation, "option B holds still before the latch");
  const latched = sample(100 + duration);
  near(Math.abs(latched.left.rotation - held.left.rotation), Math.PI / 18, "final latch is ten degrees");
  near(Math.abs(packed.left.rotation - latched.left.rotation), Math.PI / 9, "packed trails stay behind the carriage");
  entity.setupState = "deployed";
  near(sample(4200).left.rotation, latched.left.rotation, "deployment introduces no pose pop");
  entity.setupState = "tearing_down";
  sample(4300);
  const folded = sample(4300 + ANTI_TANK_GUN_TEARDOWN_TICKS / TICK_HZ * 1000);
  near(folded.visual.frameProgress, 0, "folding finishes at authoritative teardown duration");
  near(folded.left.rotation, packed.left.rotation, "teardown restores the travelling pose");
  entity.setupState = "packed"; entity.state = "move"; entity.facing = -2; entity.weaponFacing = 2;
  const moved = sample(6000);
  near(moved.left.rotation - packed.left.rotation, -2.7, "packed trails turn with the carriage while travelling");
  animation.destroy();
}
