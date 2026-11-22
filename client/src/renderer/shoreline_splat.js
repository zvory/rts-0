/**
 * Texture splatting: stable, noisy material masks feather soil and gravel over
 * the existing terrain. Only the wetness mask changes with time; the material
 * grain stays anchored in world space.
 *
 * The shared shoreline layer supplies the GLSL header and shore helpers.
 */
export const SHORELINE_SPLAT_FRAGMENT = `
vec3 splatPebbles(vec2 p, vec3 ground, float bank) {
  vec2 cell = floor(p / 11.0);
  vec3 color = ground;
  // Adjacent cells keep stones intact across cell boundaries.
  for (int y = -1; y <= 1; y++) {
    for (int x = -1; x <= 1; x++) {
      vec2 id = cell + vec2(float(x), float(y));
      float seed = shoreHash(id + 41.0);
      if (seed < 0.78 || bank < -24.0 || bank > 5.0) continue;
      vec2 root = (id + vec2(shoreHash(id + 17.0), shoreHash(id + 29.0))) * 11.0;
      vec2 local = p - root;
      float radius = 1.5 + shoreHash(id + 9.0) * 2.0;
      float shape = length(local * vec2(0.85, 1.4));
      float stone = 1.0 - smoothstep(radius - 0.8, radius + 0.3, shape);
      float shadow = (1.0 - smoothstep(radius, radius + 1.7,
        length((local - vec2(0.8, 1.0)) * vec2(0.85, 1.4)))) * 0.25;
      color *= 1.0 - shadow;
      vec3 rock = mix(vec3(0.26, 0.25, 0.20), vec3(0.48, 0.45, 0.34), seed);
      rock *= 0.90 + 0.18 * clamp(-local.y / radius, -1.0, 1.0);
      color = mix(color, rock, stone * 0.85);
    }
  }
  return color;
}

void main() {
  vec2 p = floor(vWorld / 1.5) * 1.5;
  float bank = shoreDistance(p);
  if (bank < -43.0 || bank > 29.0) discard;

  float broad = shoreNoise(p / 57.0);
  float clumps = shoreNoise(p / 13.0);
  float grain = shoreHash(floor(p / 1.5));
  // Height-like noise erodes the outer edge of each material into small islands
  // instead of producing a uniform translucent brown outline.
  float height = broad * 0.57 + clumps * 0.43;
  float width = 24.0 + broad * 12.0;
  float landMask = smoothstep(-width, -4.0, bank + (clumps - 0.5) * 8.0);
  float waterMask = 1.0 - smoothstep(3.0, 19.0 + broad * 6.0, bank);
  float cover = landMask * waterMask;

  vec3 drySoil = mix(vec3(0.36, 0.29, 0.18), vec3(0.55, 0.46, 0.29), height);
  drySoil += (grain - 0.5) * vec3(0.055, 0.044, 0.025);
  // Fine light grains remain static, like a small repeating soil texture.
  float grit = step(0.965, grain) * (1.0 - smoothstep(-5.0, 5.0, bank));
  drySoil += grit * vec3(0.065, 0.059, 0.035);

  float pulse = sin(uTime * 1.05 + shoreNoise(p / 120.0) * 4.0);
  float wetFront = -8.0 + pulse * 2.5 + (clumps - 0.5) * 5.0;
  float wet = smoothstep(wetFront - 5.0, wetFront + 3.0, bank);
  vec3 wetSoil = drySoil * vec3(0.65, 0.70, 0.72);
  vec3 color = mix(drySoil, wetSoil, wet * 0.85);
  color = splatPebbles(p, color, bank);

  // A translucent sediment mask dissolves into the existing animated water;
  // there is no separate bright contour or synthetic foam stripe.
  float submerged = smoothstep(0.0, 17.0, bank);
  vec3 sediment = mix(vec3(0.25, 0.27, 0.20), vec3(0.30, 0.30, 0.22), height);
  color = mix(color, sediment, submerged * 0.8);
  float sheen = wet * (1.0 - submerged) * smoothstep(0.65, 0.88, clumps)
    * (0.5 + 0.5 * pulse) * 0.035;
  color += vec3(sheen * 0.8, sheen, sheen * 0.75);
  float alpha = clamp(cover * 0.98, 0.0, 1.0);
  finalColor = vec4(color * uTint * alpha, alpha);
}
`;
