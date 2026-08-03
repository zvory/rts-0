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
  finalColor = vec4(vec3(outlineAlpha), outlineAlpha);
}
`;

/** Derive a white outer edge from the alpha of the actual rendered unit rig. */
export function createUnitOutlineFilter(pixi = globalThis.PIXI) {
  if (
    typeof pixi?.Filter !== "function"
    || typeof pixi?.GlProgram?.from !== "function"
    || typeof pixi?.UniformGroup !== "function"
  ) {
    throw new Error("Pixi unit outlines require Filter, GlProgram, and UniformGroup support");
  }
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
      }),
    },
  });
  filter.padding = 3;
  filter.antialias = "on";
  return filter;
}
