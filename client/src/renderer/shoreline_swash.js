/**
 * Distance-contour shoreline study. A static bank receives a moving waterline,
 * lingering damp sand, and broken wavelets travelling toward the bank. The
 * shared field supplies distance on both sides of the authoritative tile edge;
 * the animation affects presentation only.
 */
export const SHORELINE_SWASH_FRAGMENT = `
void main() {
  vec2 p = floor(vWorld / 2.0) * 2.0;
  float coarse = shoreNoise(p / 49.0);
  float detail = shoreNoise(p / 9.0);
  float grain = shoreHash(floor(p / 2.0));
  // Keep the little coves fixed in world space while the water runs over them.
  float d = shoreDistance(p) + (coarse - .5) * 5.0 + (detail - .5) * 1.8;
  float bankWidth = 24.0 + (shoreNoise(p / 83.0) - .5) * 9.0;
  float outerBank = smoothstep(-bankWidth - 5.0, -bankWidth + 2.0, d);
  float innerBank = 1.0 - smoothstep(13.0, 24.0, d);
  float bankMask = outerBank * innerBank;
  if (bankMask < .001) discard;

  // Calm pond swash: a six-second cycle moves a few pixels across the bank.
  // Broad spatial phase variation prevents the entire shoreline breathing as one.
  float phase = uTime * 1.05 + shoreNoise(p / 160.0) * 2.6;
  float runup = .5 + .5 * sin(phase);
  float front = -1.5 - runup * 7.0;
  float waterCoverage = smoothstep(front - 1.1, front + 1.5, d);
  // The maximum recent reach remains dark between waves, suggesting wet sand.
  float damp = smoothstep(-16.0, -7.0, d);
  vec3 drySand = mix(vec3(.43, .365, .235), vec3(.56, .485, .325), coarse);
  drySand += (grain - .5) * .035 + (detail - .5) * .045;
  vec3 wetSand = mix(vec3(.285, .275, .205), vec3(.365, .335, .235), detail);
  vec3 bank = mix(drySand, wetSand, damp * .83);

  // Sparse darker grains and tiny pebbles remain stationary through every wave.
  vec2 pebbleCell = floor(p / 7.0);
  vec2 pebbleCenter = (pebbleCell + vec2(.25 + shoreHash(pebbleCell) * .5,
    .25 + shoreHash(pebbleCell + 37.0) * .5)) * 7.0;
  float pebble = (1.0 - smoothstep(.8, 1.9, length((p - pebbleCenter) * vec2(1.0, 1.45))))
    * step(.79, shoreHash(pebbleCell + 11.0));
  bank = mix(bank, vec3(.27, .255, .19), pebble * .65);

  vec3 shallows = mix(vec3(.28, .30, .245), vec3(.225, .27, .23),
    smoothstep(-2.0, 18.0, d));
  shallows += (detail - .5) * .018;
  vec3 color = mix(bank, shallows, waterCoverage * .87);

  // A restrained broken pale lip follows the animated contour, with a second
  // thin wave moving inward over the existing animated water material.
  float lip = 1.0 - smoothstep(.5, 1.7, abs(d - front - .9));
  float lipBreak = smoothstep(.26, .66, shoreNoise(p / 13.0 + vec2(0.0, uTime * .035)));
  float wavePosition = mod(uTime * 3.2 + 5.0, 19.0);
  float wave = 1.0 - smoothstep(.5, 1.35, abs(d - (18.0 - wavePosition)));
  wave *= smoothstep(1.0, 5.0, d) * (1.0 - smoothstep(14.0, 22.0, d));
  float foam = max(lip * lipBreak * .48, wave * lipBreak * .22);
  color = mix(color, vec3(.62, .625, .505), foam);
  // The water-side opacity falls away, leaving the original caustics and ripples.
  float alpha = bankMask * mix(.96, .60, waterCoverage);
  finalColor = vec4(color * uTint * alpha, alpha);
}
`;
