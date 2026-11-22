/**
 * A procedural 16-case corner autotile set. Each dual-grid cell selects a
 * straight edge, a round outer corner, a round inner corner, or a saddle pair.
 * No interpolated distance texture drives this variant: its shapes come only
 * from the four neighboring terrain bits, like a conventional transition atlas.
 */
export const SHORELINE_TILES_FRAGMENT = `
float cornerTileDistance(vec2 p, int mask) {
  if (mask == 0) return -2.;
  if (mask == 15) return 2.;
  // Bit order: top-left, top-right, bottom-right, bottom-left.
  if (mask == 1) return .5-length(p);
  if (mask == 2) return .5-length(p-vec2(1,0));
  if (mask == 4) return .5-length(p-vec2(1,1));
  if (mask == 8) return .5-length(p-vec2(0,1));
  if (mask == 3) return .5-p.y;
  if (mask == 6) return p.x-.5;
  if (mask == 12) return p.y-.5;
  if (mask == 9) return .5-p.x;
  if (mask == 14) return length(p)-.5;
  if (mask == 13) return length(p-vec2(1,0))-.5;
  if (mask == 11) return length(p-vec2(1,1))-.5;
  if (mask == 7) return length(p-vec2(0,1))-.5;
  // Opposite corners stay disconnected rather than inventing a diagonal bridge.
  if (mask == 5) return .5-min(length(p),length(p-vec2(1,1)));
  return .5-min(length(p-vec2(1,0)),length(p-vec2(0,1)));
}

void main() {
  vec2 p=floor(vWorld/2.)*2.;
  vec2 cell=floor(p/uTileSize-.5);
  vec2 local=p/uTileSize-.5-cell;
  vec2 origin=(cell+.5)*uTileSize;
  int mask=int(shoreWater(origin))
    +2*int(shoreWater(origin+vec2(uTileSize,0)))
    +4*int(shoreWater(origin+vec2(uTileSize,uTileSize)))
    +8*int(shoreWater(origin+vec2(0,uTileSize)));
  float d=cornerTileDistance(local,mask)*uTileSize;
  // Uniform neighbors have no transition artwork. End both sides of the
  // transition before a mixed cell reaches their shared +/-half-tile boundary.
  float support=min(14.,uTileSize*.4375);
  if (abs(d)>=support) { finalColor=vec4(0); return; }

  // Tile art is stable in world space; moving surf never crawls across the bank.
  float grain=shoreHash(floor(p/2.));
  float broad=shoreNoise(p/27.);
  float sandEdge=-support*.72+(shoreNoise(p/11.)-.5)*support*.10;
  float bank=smoothstep(sandEdge-support*.14,sandEdge+support*.14,d);
  float underwater=smoothstep(1.,12.,d);
  vec3 dry=mix(vec3(.40,.355,.245),vec3(.60,.53,.365),broad*.65+grain*.35);
  vec3 wet=mix(vec3(.245,.235,.17),vec3(.34,.31,.225),grain*.7+broad*.3);
  vec3 sand=mix(dry,wet,smoothstep(-7.,2.,d));
  vec3 shallows=vec3(.265,.285,.225)+(grain-.5)*.035;
  vec3 color=mix(sand,shallows,underwater);

  // Small pebble stamps in the dry part of the transition tile.
  vec2 pebbleCell=floor(p/9.);
  vec2 pebbleCenter=(pebbleCell+vec2(.2)+vec2(shoreHash(pebbleCell),shoreHash(pebbleCell+8.))*.6)*9.;
  float pebble=1.-smoothstep(1.2,2.6,length((p-pebbleCenter)*vec2(1.,1.4)));
  pebble*=step(.74,shoreHash(pebbleCell+32.))*(1.-smoothstep(-7.,-1.,d));
  color=mix(color,vec3(.405,.395,.335),pebble*.7);

  // A narrow irregular foam lip laps around the chosen corner/edge shape.
  float phase=uTime*1.25+shoreNoise(p/48.)*2.4;
  float lip=2.5+sin(phase)*2.;
  float foam=(1.-smoothstep(.6,2.,abs(d-lip)))*smoothstep(.32,.65,shoreNoise(p/8.+uTime*.035));
  color=mix(color,vec3(.67,.68,.54),foam*.52);
  float alpha=bank*(1.-smoothstep(support*.50,support*.86,d));
  finalColor=vec4(color*uTint*alpha,alpha);
}
`;
