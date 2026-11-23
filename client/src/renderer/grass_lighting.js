// World-space atmosphere shared by procedural and atlas grass. Keep blade animation separate.
export const GRASS_LIGHTING_GLSL = `float hash(vec2 p){ return fract(sin(dot(p,vec2(127.1,311.7)))*43758.5453); }
float noise(vec2 p){
 vec2 i=floor(p), f=fract(p); f=f*f*(3.-2.*f);
 return mix(mix(hash(i),hash(i+vec2(1,0)),f.x),mix(hash(i+vec2(0,1)),hash(i+vec2(1)),f.x),f.y);
}
// Wide, curved fronts travel across the field about twenty seconds apart.
float windLight(vec2 p){
 float x=p.x-uTime*80.;
 float cell=floor(x/1600.);
 float light=0.;
 for(int i=-1;i<=1;i++){
   float id=cell+float(i);
   float center=(id+.5)*1600.+(hash(vec2(id,7.))-.5)*350.;
   float curve=(noise(vec2(p.y/550.,id+91.))-.5)*350.;
   float width=190.+hash(vec2(id,11.))*120.;
   float d=(x-center+curve)/width;
   light+=exp(-d*d*1.5)*(.24+hash(vec2(id,17.))*.12);
 }
 return light;
}
vec3 grassAtmosphere(vec2 p, vec3 color, float frontStrength){
 float cloud=smoothstep(.36,.68,noise(p/250.+vec2(-uTime*.028,uTime*.009)));
 color*=mix(.80,1.09,cloud);
 color*=1.+windLight(p)*frontStrength;
 return color;
}
`;
