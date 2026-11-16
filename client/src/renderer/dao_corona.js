const VERTEX = `
in vec2 aPosition;
out vec2 vLocal;
uniform vec4 uOutputFrame;
uniform vec4 uOutputTexture;
void main() {
 vec2 p = aPosition * uOutputFrame.zw + uOutputFrame.xy;
 p.x = p.x * (2.0 / uOutputTexture.x) - 1.0;
 p.y = p.y * (2.0 * uOutputTexture.z / uOutputTexture.y) - uOutputTexture.z;
 gl_Position = vec4(p, 0.0, 1.0);
 vLocal = aPosition * 2.0 - 1.0;
}`;
const FRAGMENT = `
in vec2 vLocal;
out vec4 finalColor;
uniform float uTime;
uniform float uStrength;
float hash(vec2 p) { return fract(sin(dot(p,vec2(127.1,311.7)))*43758.5453); }
float noise(vec2 p) {
 vec2 i=floor(p), f=fract(p); f=f*f*(3.0-2.0*f);
 return mix(mix(hash(i),hash(i+vec2(1,0)),f.x),mix(hash(i+vec2(0,1)),hash(i+vec2(1,1)),f.x),f.y);
}
float fbm(vec2 p) { return 0.6*noise(p)+0.28*noise(p*2.03)+0.12*noise(p*4.07); }
void main() {
 vec2 p=vLocal*32.0;
 float r=length(p), a=atan(p.y,p.x), t=uTime;
 float pulse=0.86+0.14*sin(t*2.2);
 float n=fbm(p*0.18+vec2(t*0.21,t*0.13));
 float shell=r-(13.6+(n-0.5)*2.0);
 float heat=0.6+0.4*fbm(vec2(cos(a-t*0.35),sin(a-t*0.35))*3.0);
 float light=(0.76*exp(-shell*shell/5.6)+0.34*exp(-shell*shell/38.0))*heat*pulse;
 vec3 gold=mix(vec3(1.0,0.64,0.13),vec3(1.0,0.97,0.79),exp(-shell*shell/5.6)*0.8);
 light*=(1.0-smoothstep(24.0,31.0,r))*uStrength;
 float alpha=clamp(light,0.0,0.85);
 finalColor=vec4(gold*alpha,alpha);
}`;

/** Tight Corona: a close, softly blooming light field around a ready Disciple. */
export function createDaoCorona(pixi = globalThis.PIXI) {
  return new (class DaoCorona extends pixi.Graphics {
    update(entity, cy, now) {
      const advancing = entity.daoAdvanceTicks > 0;
      this.visible = !!entity.daoReady || advancing;
      if (!this.visible) return;
      if (!this.coronaFilter) {
        this.coronaUniforms = new pixi.UniformGroup({
          uTime: { value: 0, type: "f32" },
          uStrength: { value: 1, type: "f32" },
        });
        this.coronaFilter = new pixi.Filter({
          glProgram: pixi.GlProgram.from({ vertex: VERTEX, fragment: FRAGMENT, name: "dao-tight-corona" }),
          resources: { coronaUniforms: this.coronaUniforms },
          // The shader maps the entire filter quad to a centered corona. Cropping
          // that quad at the viewport edge would recenter and squash the glow.
          clipToViewport: false,
        });
        this.coronaFilter.padding = 0;
        this.filters = [this.coronaFilter];
        this.blendMode = "add";
      }
      const scale = advancing ? 1 + (1 - entity.daoAdvanceTicks / 15) : 1;
      this.clear().rect(-32 * scale, cy - 32 * scale, 64 * scale, 64 * scale).fill(0xffffff);
      this.coronaUniforms.uniforms.uTime = now / 1000;
      this.coronaUniforms.uniforms.uStrength = advancing ? Math.min(1.4, entity.daoAdvanceTicks / 5) : 1;
    }

    destroy(options) {
      this.coronaFilter?.destroy();
      this.coronaFilter = null;
      super.destroy(options);
    }
  })();
}
