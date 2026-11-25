import { ABILITY, KIND } from "../protocol.js";

const CULTIVATOR_FIGHTERS = new Set([
  KIND.WARRIOR,
  KIND.WIND_ADEPT,
  KIND.WAR_ADEPT,
  KIND.MAGIC_ADEPT,
]);

/** Count occupied item categories from authorized snapshot state, including active buffs. */
export function cultivatorItemTier(entity) {
  if (!CULTIVATOR_FIGHTERS.has(entity?.kind)) return 0;
  let defense = false,
    attack = false;
  for (const entry of entity.abilities || []) {
    if (!(entry.remainingUses > 0 || entry.cooldownLeft > 0)) continue;
    if (
      entry.ability === ABILITY.ARMOR_TREASURE ||
      entry.ability === ABILITY.VITALITY_TREASURE
    )
      defense = true;
    if (
      entry.ability === ABILITY.HASTE ||
      entry.ability === ABILITY.DOUBLE_DAMAGE
    )
      attack = true;
  }
  return Number(!!entity.escapeTreasure) + Number(defense) + Number(attack);
}
const vertex = `
in vec2 aPosition;
out vec2 vTextureCoord;
uniform highp vec4 uInputSize;
uniform vec4 uOutputFrame;
uniform vec4 uOutputTexture;
void main() {
 vec2 p=aPosition*uOutputFrame.zw+uOutputFrame.xy;
 p.x=p.x*2.0/uOutputTexture.x-1.0;
 p.y=p.y*2.0*uOutputTexture.z/uOutputTexture.y-uOutputTexture.z;
 gl_Position=vec4(p,0,1);
 vTextureCoord=aPosition*uOutputFrame.zw*uInputSize.zw;
}`;
const fragment = `
in vec2 vTextureCoord;
out vec4 finalColor;
uniform sampler2D uTexture;
uniform highp vec4 uInputSize;
uniform float uTime;
uniform float uTier;
void main() {
 vec4 body=texture(uTexture,vTextureCoord);
 float pulse=uTier>3.5 ? 0.8+0.2*sin(uTime*3.0) : 0.65+0.35*sin(uTime*3.0);
 float rim=0.0, bloom=0.0, rays=0.0;
 for(int i=0;i<32;i++) {
  float a=float(i)*6.2831853/32.0;
  vec2 d=vec2(cos(a),sin(a));
  rim=max(rim,texture(uTexture,vTextureCoord+d*uInputSize.zw*(0.65+uTier*0.45)).a);
  bloom+=texture(uTexture,vTextureCoord+d*uInputSize.zw*(1.5+uTier*1.4)).a/32.0;
  if(uTier>2.5) {
   float reach=(uTier/3.0)*(6.0+7.0*pow(0.5+0.5*sin(a*7.0+uTime*0.65),8.0));
   rays+=texture(uTexture,vTextureCoord+d*uInputSize.zw*reach).a/32.0;
  }
 }
 float edge=max(0.0,rim-body.a);
 float glow=(edge*(0.32+0.16*uTier)+bloom*(0.2+0.18*uTier)+rays*(1.6+0.45*(uTier-3.0)))*pulse*(1.0+0.15*(uTier-3.0));
 if(uTier>3.5) glow=(edge*1.05+bloom*0.85+rays*0.9)*pulse;
 float alpha=clamp(glow,0.0,0.95)*(1.0-body.a);
 vec3 gold=mix(vec3(1.0,0.58,0.10),vec3(1.0,0.93,0.62),edge*0.7);
 finalColor=body+vec4(gold*alpha,alpha);
}`;
export function updateItemRadiance(instance, entity, now) {
  if (instance.silhouetteShadow) return;
  const tier = cultivatorItemTier(entity);
  if (instance.itemGlow) instance.itemGlow.visible = tier >= 2;
  if (tier >= 2) {
    if (!instance.itemGlow) {
      instance.itemGlow = new PIXI.Sprite(instance.sprite.texture);
      instance.itemGlow.anchor.set(0.5);
      instance.itemGlow.tint = 0xffc34f;
      instance.itemGlow.blendMode = "add";
      instance.itemGlowBlur = new PIXI.BlurFilter({ strength: 4, quality: 6 });
      instance.itemGlow.filters = [instance.itemGlowBlur];
      instance.container.addChildAt(instance.itemGlow, 0);
    }
    const glow = instance.itemGlow;
    glow.texture = instance.sprite.texture;
    glow.position.copyFrom(instance.sprite.position);
    glow.scale.copyFrom(instance.sprite.scale);
    instance.itemGlowBlur.strength = tier === 2 ? 3 : 6;
    glow.alpha =
      (tier === 2 ? 0.6 : 0.95) * (0.8 + 0.2 * Math.sin((now / 1000) * 3));
  }
  if (instance.itemRays) instance.itemRays.visible = tier > 0;
  if (tier > 0) {
    if (!instance.itemRays) {
      instance.itemRays = new PIXI.Graphics();
      instance.container.addChildAt(instance.itemRays, 0);
    }
    const g = instance.itemRays,
      t = now / 1000;
    g.clear();
    const rayCount = 12 + (tier - 1) * 4;
    const boost = 1 + (tier - 1) * 0.4;
    for (let i = 0; i < rayCount; i++) {
      const a = (i * Math.PI * 2) / rayCount + t * 0.1;
      const pulse = 0.5 + 0.5 * Math.sin(t * 3 + i * 0.65);
      const start = 16,
        end = (28 + 10 * pulse) * boost;
      const dx = Math.cos(a),
        dy = Math.sin(a),
        w = (0.5 + 0.3 * pulse) * (1 + (tier - 1) * 0.7);
      g.moveTo(dx * start - dy * w, dy * start + dx * w)
        .lineTo(dx * end, dy * end)
        .lineTo(dx * start + dy * w, dy * start - dx * w)
        .closePath()
        .fill({
          color: 0xffd36d,
          alpha: Math.min(0.85, (0.18 + 0.32 * pulse) * (1 + (tier - 1) * 0.5)),
        });
    }
  }
  if (!tier) {
    if (instance.itemRim) instance.sprite.filters = null;
    return;
  }
  if (!instance.itemRim) {
    instance.itemRimUniforms = new PIXI.UniformGroup({
      uTime: { value: 0, type: "f32" },
      uTier: { value: 1, type: "f32" },
    });
    instance.itemRim = new PIXI.Filter({
      glProgram: PIXI.GlProgram.from({
        vertex,
        fragment,
        name: "cultivator-item-radiance",
      }),
      resources: { rim: instance.itemRimUniforms },
      padding: 48,
    });
  }
  instance.sprite.filters = [instance.itemRim];
  instance.itemRimUniforms.uniforms.uTime = now / 1000;
  instance.itemRimUniforms.uniforms.uTier = 3 + (tier - 1) * 2;
}

/** Filters own GPU resources independently of the parent display-object tree. */
export function destroyItemRadiance(instance) {
  if (instance.itemRim) instance.sprite.filters = null;
  if (instance.itemGlow) instance.itemGlow.filters = null;
  instance.itemRim?.destroy();
  instance.itemGlowBlur?.destroy();
  instance.itemRim = null;
  instance.itemGlowBlur = null;
}
