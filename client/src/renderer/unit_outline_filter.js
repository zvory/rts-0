const UNIT_OUTLINE_VERTEX = `
in vec2 aPosition;
out vec2 vTextureCoord;

uniform highp vec4 uInputSize;
uniform vec4 uOutputFrame;
uniform vec4 uOutputTexture;

vec4 filterVertexPosition(void) {
  vec2 position = aPosition * uOutputFrame.zw + uOutputFrame.xy;
  position.x = position.x * (2.0 / uOutputTexture.x) - 1.0;
  position.y = position.y * (2.0 * uOutputTexture.z / uOutputTexture.y) - uOutputTexture.z;
  return vec4(position, 0.0, 1.0);
}

vec2 filterTextureCoord(void) {
  return aPosition * (uOutputFrame.zw * uInputSize.zw);
}

void main(void) {
  gl_Position = filterVertexPosition();
  vTextureCoord = filterTextureCoord();
}
`;

const UNIT_OUTLINE_FRAGMENT = `
in vec2 vTextureCoord;
out vec4 finalColor;

uniform sampler2D uTexture;
uniform highp vec4 uInputSize;
uniform float uThickness;
uniform float uAlpha;
uniform vec3 uFillColor;
uniform float uFillAlpha;

void main(void) {
  vec2 texel = uInputSize.zw * uThickness;
  float centerAlpha = texture(uTexture, vTextureCoord).a;
  float neighborAlpha = 0.0;
  neighborAlpha = max(neighborAlpha, texture(uTexture, vTextureCoord + vec2( texel.x, 0.0)).a);
  neighborAlpha = max(neighborAlpha, texture(uTexture, vTextureCoord + vec2(-texel.x, 0.0)).a);
  neighborAlpha = max(neighborAlpha, texture(uTexture, vTextureCoord + vec2(0.0,  texel.y)).a);
  neighborAlpha = max(neighborAlpha, texture(uTexture, vTextureCoord + vec2(0.0, -texel.y)).a);
  neighborAlpha = max(neighborAlpha, texture(uTexture, vTextureCoord + vec2( texel.x,  texel.y)).a);
  neighborAlpha = max(neighborAlpha, texture(uTexture, vTextureCoord + vec2(-texel.x,  texel.y)).a);
  neighborAlpha = max(neighborAlpha, texture(uTexture, vTextureCoord + vec2( texel.x, -texel.y)).a);
  neighborAlpha = max(neighborAlpha, texture(uTexture, vTextureCoord + vec2(-texel.x, -texel.y)).a);
  float outlineAlpha = max(0.0, neighborAlpha - centerAlpha) * uAlpha;
  float fillAlpha = centerAlpha * uFillAlpha;
  vec4 fill = vec4(uFillColor * fillAlpha, fillAlpha);
  vec4 outline = vec4(vec3(outlineAlpha), outlineAlpha);
  finalColor = outline + fill * (1.0 - outlineAlpha);
}
`;

export const FOREST_UNIT_FILL_ALPHA = 0.85;

/** Derive a white outer edge and optional flat fill from the rendered unit's alpha. */
export function createUnitOutlineFilter(pixi = globalThis.PIXI, options = {}) {
  if (
    typeof pixi?.Filter !== "function"
    || typeof pixi?.GlProgram?.from !== "function"
    || typeof pixi?.UniformGroup !== "function"
  ) {
    throw new Error("Pixi unit outlines require Filter, GlProgram, and UniformGroup support");
  }
  const fillColor = Number.isFinite(options.fillColor) ? options.fillColor : 0xffffff;
  const fillAlpha = Math.max(0, Math.min(1, Number(options.fillAlpha) || 0));
  const filter = new pixi.Filter({
    glProgram: pixi.GlProgram.from({
      vertex: UNIT_OUTLINE_VERTEX,
      fragment: UNIT_OUTLINE_FRAGMENT,
      name: "unit-alpha-outline",
    }),
    resources: {
      outlineUniforms: new pixi.UniformGroup({
        uThickness: { value: 1.65, type: "f32" },
        uAlpha: { value: 0.96, type: "f32" },
        uFillColor: {
          value: new Float32Array([
            ((fillColor >> 16) & 0xff) / 255,
            ((fillColor >> 8) & 0xff) / 255,
            (fillColor & 0xff) / 255,
          ]),
          type: "vec3<f32>",
        },
        uFillAlpha: { value: fillAlpha, type: "f32" },
      }),
    },
  });
  filter.padding = 3;
  filter.antialias = "on";
  return filter;
}
