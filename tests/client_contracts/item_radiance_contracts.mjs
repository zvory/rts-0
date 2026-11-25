import {
  prepareEntitySnapshots,
  preparedPresentationEntityRecord,
} from "../../client/src/presentation/entity_snapshot.js";
import { assert } from "./assertions.mjs";
import { ABILITY, KIND } from "../../client/src/protocol.js";
import {
  cultivatorItemTier,
  updateItemRadiance,
  destroyItemRadiance,
} from "../../client/src/renderer/item_radiance.js";

for (const kind of [
  KIND.WARRIOR,
  KIND.WIND_ADEPT,
  KIND.WAR_ADEPT,
  KIND.MAGIC_ADEPT,
]) {
  for (const daoMaster of [false, true]) {
    for (let tier = 0; tier <= 3; tier++) {
      const publicView = { kind, daoMaster, itemRadianceTier: tier };
      assert(
        cultivatorItemTier(publicView) === tier,
        "enemies render the public tier without inventory",
      );
      const prepared = prepareEntitySnapshots([publicView]).entries[0];
      const presented = preparedPresentationEntityRecord(prepared, {
        type: "unit",
        x: 100,
        y: 100,
        owner: 2,
        relationship: "enemy",
        visualBounds: {},
        anchors: {},
      });
      assert(
        cultivatorItemTier(presented) === tier,
        "public item tier survives the real render snapshot boundary without private inventory",
      );
      assert(
        cultivatorItemTier({
          ...publicView,
          escapeTreasure: true,
          abilities: [{ ability: ABILITY.HASTE, remainingUses: 1 }],
        }) === tier,
        "private owner fields cannot change the public presentation",
      );
    }
  }
}
for (const itemRadianceTier of [undefined, null, -1, 4, 1.5, "3", NaN]) {
  assert(
    cultivatorItemTier({ kind: KIND.MAGIC_ADEPT, itemRadianceTier }) === 0,
    "missing or invalid visual tiers fail closed",
  );
}
assert(
  cultivatorItemTier({ kind: KIND.SHOP, itemRadianceTier: 3 }) === 0,
  "Shop cannot gain fighter radiance",
);
assert(
  cultivatorItemTier({ kind: KIND.RIFLEMAN, itemRadianceTier: 3 }) === 0,
  "other faction presentation is unchanged",
);
assert(
  cultivatorItemTier({
    kind: KIND.MAGIC_ADEPT,
    escapeTreasure: true,
    abilities: [{ ability: ABILITY.HASTE, remainingUses: 1 }],
  }) === 0,
  "private state alone cannot invent a public glow",
);

let disposed = 0;
const instance = {
  sprite: { filters: ["rim"] },
  itemGlow: { visible: true, filters: ["blur"] },
  itemRays: { visible: true },
  itemRim: {
    destroy() {
      disposed++;
    },
  },
  itemGlowBlur: {
    destroy() {
      disposed++;
    },
  },
};
updateItemRadiance(instance, { kind: KIND.MAGIC_ADEPT }, 0);
assert(
  instance.sprite.filters === null &&
    !instance.itemGlow.visible &&
    !instance.itemRays.visible,
  "inventory loss clears all radiance components immediately",
);
destroyItemRadiance(instance);
destroyItemRadiance(instance);
assert(
  disposed === 2 && instance.itemGlow.filters === null,
  "GPU filters dispose once and detach",
);
updateItemRadiance(
  { silhouetteShadow: true },
  { kind: KIND.MAGIC_ADEPT, escapeTreasure: true },
  0,
);

// Exercise positive tiers as well as removal: auxiliary sprites must follow the animated body.
const previousPixi = globalThis.PIXI;
let createdFilters = 0;
const point = () => ({
  copyFrom(source) {
    this.copied = source;
  },
});
class Sprite {
  constructor(texture) {
    this.texture = texture;
    this.anchor = { set() {} };
    this.position = point();
    this.scale = point();
  }
}
class Graphics {
  clear() {
    return this;
  }
  moveTo() {
    return this;
  }
  lineTo() {
    return this;
  }
  closePath() {
    return this;
  }
  fill() {
    return this;
  }
}
class Filter {
  constructor(options) {
    this.options = options;
    createdFilters++;
  }
  destroy() {
    this.destroyed = true;
  }
}
try {
  globalThis.PIXI = {
    Sprite,
    Graphics,
    Filter,
    BlurFilter: Filter,
    UniformGroup: class {
      constructor(entries) {
        this.uniforms = Object.fromEntries(
          Object.entries(entries).map(([k, v]) => [k, v.value]),
        );
      }
    },
    GlProgram: {
      from(options) {
        return options;
      },
    },
  };
  const body = new Sprite("idle-frame");
  const live = { sprite: body, container: { addChildAt() {} } };
  const unit = { kind: KIND.MAGIC_ADEPT, itemRadianceTier: 1 };
  updateItemRadiance(live, unit, 1000);
  assert(
    live.itemRimUniforms.uniforms.uTier === 3 && !live.itemGlow,
    "one item starts at the approved former maximum without allocating bloom",
  );
  unit.itemRadianceTier = 2;
  updateItemRadiance(live, unit, 1100);
  const rim = live.itemRim,
    blur = live.itemGlowBlur;
  assert(
    live.itemRimUniforms.uniforms.uTier === 5 &&
      live.itemGlow.visible !== false,
    "two items add bloom and increase rim strength",
  );
  unit.itemRadianceTier = 3;
  body.texture = "moving-frame";
  updateItemRadiance(live, unit, 1200);
  assert(
    live.itemGlow.texture === "moving-frame" &&
      live.itemGlow.position.copied === body.position &&
      live.itemGlow.scale.copied === body.scale,
    "bloom follows the current frame and pose",
  );
  assert(
    live.itemRimUniforms.uniforms.uTier === 7 &&
      live.itemGlowBlur.strength === 6,
    "three items use the strongest rim and bloom",
  );
  assert(
    live.itemRim === rim && live.itemGlowBlur === blur && createdFilters === 2,
    "animated updates reuse GPU filters",
  );
  unit.itemRadianceTier = 0;
  updateItemRadiance(live, unit, 1300);
  assert(
    !live.itemGlow.visible && !live.itemRays.visible && body.filters === null,
    "all allocated components hide when the final item disappears",
  );
  destroyItemRadiance(live);
  assert(
    rim.destroyed && blur.destroyed,
    "allocated filters are destroyed at teardown",
  );
} finally {
  if (previousPixi === undefined) delete globalThis.PIXI;
  else globalThis.PIXI = previousPixi;
}
