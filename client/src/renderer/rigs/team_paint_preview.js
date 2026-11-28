// Local idle-art experiment: one neutral base and one shaded paint mask share an atlas.
const VERTEX = `#version 300 es
precision highp float;
in vec2 aPosition;
in vec2 aUV;
uniform mat3 uProjectionMatrix;
uniform mat3 uWorldTransformMatrix;
uniform mat3 uTransformMatrix;
out vec2 vUV;
void main(){vUV=aUV;gl_Position=vec4((uProjectionMatrix*uWorldTransformMatrix*uTransformMatrix*vec3(aPosition,1.)).xy,0.,1.);}`;
const FRAGMENT = `#version 300 es
precision highp float;
in vec2 vUV;
out vec4 finalColor;
uniform sampler2D uAtlas;
uniform vec2 uCellSize;
uniform vec3 uTeam;
uniform vec3 uSun;
void main(){
 vec2 uv=vUV*uCellSize;
 vec4 base=texture(uAtlas,uv);
 float paint=texture(uAtlas,uv+vec2(uCellSize.x,0.)).r;
 finalColor=vec4((base.rgb+paint*uTeam)*uSun,base.a);
}`;
const PALETTE=[0xe84b4b,0x378ce9,0x32c77c,0xf2d84b,0xba6bea,0xf29138,0xe9edf2];
export function previewColorForEntity(entity){return PALETTE[Math.max(0,Math.min(6,Math.round((entity.x-1600)/144)))];}
const rgb=value=>new Float32Array([((value>>16)&255)/255,((value>>8)&255)/255,(value&255)/255]);
export function isTeamPaintPreview(strip){return /\/(warrior|wind-melee-b|war-yari|magic|wind-master|war-master|magic-master)-runtime-strip\.png/.test(strip.image)}
export function createTeamPaintPreview(pixi,strip,texture,sun){
 const w=strip.frameWidth,h=strip.frameHeight;
 const geometry=new pixi.Geometry({attributes:{aPosition:{buffer:new Float32Array([-w/2,-h/2,w/2,-h/2,w/2,h/2,-w/2,h/2]),format:'float32x2'},aUV:{buffer:new Float32Array([0,0,1,0,1,1,0,1]),format:'float32x2'}},indexBuffer:new Uint32Array([0,1,2,0,2,3])});
 const uniforms=new pixi.UniformGroup({uCellSize:{value:new Float32Array([w/texture.source.width,h/texture.source.height]),type:'vec2<f32>'},uTeam:{value:rgb(PALETTE[0]),type:'vec3<f32>'},uSun:{value:rgb(sun??0xffffff),type:'vec3<f32>'}});
 const shader=pixi.Shader.from({gl:{vertex:VERTEX,fragment:FRAGMENT,name:'preview-shared-team-paint-mask'},resources:{paintUniforms:uniforms,uAtlas:texture.source}});
 const mesh=new pixi.Mesh({geometry,shader});
 return {mesh,update(entity,scale,offset){mesh.scale.set(scale,scale);mesh.position.set(offset.x,offset.y);uniforms.uniforms.uTeam.set(rgb(previewColorForEntity(entity)));},destroy(){mesh.parent?.removeChild(mesh);mesh.destroy();geometry.destroy(true);shader.destroy();}};
}
