// Palette study C: Darker and browner.
#version 300 es
// Port of jess-hammer/2d-pixel-water-shader-godot, MIT (see adjacent license).
// Preserves the generated visual shader calculations below. Changes: Godot/Pixi
// bindings, material constants, 2-world-pixel grid, and shore-derived input mask/palette.
precision highp float;
in vec2 vWorld;
out vec4 finalColor;
uniform float uTime;
uniform vec2 uMapSize;
uniform sampler2D uShore;

const float CausticTextureScale = .0009;
const float MovementSpeed = .25;
const float MovementScale = .05;
uniform sampler2D MovementNoise;
const float MovementStrength = .03;
uniform sampler2D CausticTexture;
const vec4 CausticColour = vec4(.46,.56,.46,.321569);
uniform sampler2D CausticHighlightTexture;
const vec4 CausticHighlightColour = vec4(.71,.77,.68,.792157);
const float RandomFadeSpeed = .03;
const float RandomFadeScale = .001;
uniform sampler2D RandomFadeNoise;
const float RandomFadeStrength = .6;
const float SpecularScaleMoving = .007;
const float SpecularSpeed = .03;
uniform sampler2D SpecularNoiseTextureMoving1;
uniform sampler2D SpecularNoiseTextureMoving2;
const float SpecularThreshold = .15;
const vec4 SpecularColour = vec4(.75,.80,.72,.65);
const vec4 FoamColour = vec4(0.295,0.300,0.250,.25);
const float FoamWaveSpeed = 5.0;
const float FoamNoiseScale = .00008;
uniform sampler2D FoamNoiseTexture;
const float FoamNoiseAmount = .8;
const float FoamFrequency = 18.0;
const float FoamQuantizeAmount = 3.5;



void main() {
  vec2 pixelWorld = floor(vWorld / 2.0) * 2.0;
  float shore = texture(uShore, pixelWorld / uMapSize).r * 128.0;
  // Adaptation: discrete shore-based depth bands replace Godot's painted tile mask.
  float depth = floor(clamp(shore / 52.0, 0.0, 0.999) * 4.0) / 3.0;
  vec3 waterPalette = vec3(0.245, 0.255, 0.210); // subdued sediment tint at the bank
  if (depth > .2) waterPalette = vec3(0.225, 0.250, 0.210); // quiet gray-green shallows
  if (depth > .5) waterPalette = vec3(0.205, 0.240, 0.205); // restrained pond-green middle band
  if (depth > .8) waterPalette = vec3(0.185, 0.230, 0.200); // slightly deeper, equally muted pond water
  vec4 WaterColour = vec4(waterPalette, 1.0);
  vec4 inputMask = vec4(clamp(1.0 - shore / 40.0, 0.0, .85), 0.0, 1.0, 1.0);

// ColorParameter:49
	vec4 n_out49p0 = WaterColour;


// VaryingGetter:6
	vec2 n_out6p0 = pixelWorld;


// VectorFunc:59
	vec2 n_out59p0 = floor(n_out6p0);


// FloatParameter:8
	float n_out8p0 = CausticTextureScale;


// VectorOp:7
	vec2 n_out7p0 = n_out59p0 * vec2(n_out8p0);


// Input:87
	float n_out87p0 = uTime;


// FloatParameter:89
	float n_out89p0 = MovementSpeed;


// FloatOp:203
	float n_out203p0 = n_out87p0 * n_out89p0;


// FloatParameter:210
	float n_out210p0 = MovementScale;


// VectorOp:209
	vec2 n_out209p0 = vec2(n_out210p0) * n_out59p0;


// VectorOp:205
	vec2 n_out205p0 = vec2(n_out203p0) + n_out209p0;


	vec4 n_out201p0;
// Texture2D:201
	n_out201p0 = texture(MovementNoise, n_out205p0);


// FloatParameter:94
	float n_out94p0 = MovementStrength;


// VectorOp:200
	vec2 n_out200p0 = vec2(n_out201p0.xy) * vec2(n_out94p0);


// VectorOp:92
	vec2 n_out92p0 = n_out7p0 + n_out200p0;


	vec4 n_out10p0;
// Texture2D:10
	n_out10p0 = texture(CausticTexture, n_out92p0);


// ColorParameter:41
	vec4 n_out41p0 = CausticColour;


// VectorOp:32
	vec4 n_out32p0 = n_out10p0 * n_out41p0;


	vec4 n_out44p0;
// Texture2D:44
	n_out44p0 = texture(CausticHighlightTexture, n_out92p0);


// ColorParameter:45
	vec4 n_out45p0 = CausticHighlightColour;


// VectorOp:46
	vec4 n_out46p0 = n_out44p0 * n_out45p0;
	float n_out46p4 = n_out46p0.a;


// Mix:47
	vec4 n_out47p0 = mix(n_out32p0, n_out46p0, vec4(n_out46p4));


// VectorDecompose:57
	float n_out57p0 = n_out47p0.x;
	float n_out57p1 = n_out47p0.y;
	float n_out57p2 = n_out47p0.z;
	float n_out57p3 = n_out47p0.w;


// Input:37
	float n_out37p0 = uTime;


// FloatParameter:39
	float n_out39p0 = RandomFadeSpeed;


// VaryingGetter:13
	vec2 n_out13p0 = pixelWorld;


// FloatParameter:17
	float n_out17p0 = RandomFadeScale;


// VectorOp:213
	vec2 n_out213p0 = n_out13p0 * vec2(n_out17p0);


// MultiplyAdd:212
	vec2 n_out212p0 = (vec2(n_out37p0) * vec2(n_out39p0) + n_out213p0);


	vec4 n_out214p0;
// Texture2D:214
	n_out214p0 = texture(RandomFadeNoise, n_out212p0);


// FloatParameter:18
	float n_out18p0 = RandomFadeStrength;


// FloatOp:31
	float n_out31p0 = n_out214p0.x * n_out18p0;


// FloatOp:60
	float n_out60p0 = n_out31p0 * n_out57p3;


// VectorCompose:61
	vec4 n_out61p0 = vec4(n_out57p0, n_out57p1, n_out57p2, n_out60p0);


// FloatParameter:106
	float n_out106p0 = SpecularScaleMoving;


// VaryingGetter:96
	vec2 n_out96p0 = pixelWorld;


// VectorFunc:97
	vec2 n_out97p0 = floor(n_out96p0);


// VectorOp:218
	vec2 n_out218p0 = vec2(n_out106p0) * n_out97p0;


// FloatParameter:102
	float n_out102p0 = SpecularSpeed;


// Input:103
	float n_out103p0 = uTime;


// VectorOp:101
	vec2 n_out101p0 = vec2(n_out102p0) * vec2(n_out103p0);


// VectorOp:104
	vec2 n_out104p0 = n_out218p0 - n_out101p0;


	vec4 n_out217p0;
// Texture2D:217
	n_out217p0 = texture(SpecularNoiseTextureMoving1, n_out104p0);


// VectorOp:100
	vec2 n_out100p0 = n_out218p0 + n_out101p0;


	vec4 n_out216p0;
// Texture2D:216
	n_out216p0 = texture(SpecularNoiseTextureMoving2, n_out100p0);


	vec3 n_out107p0;
// ColorOp:107
	{
		float base = vec3(n_out217p0.xyz).x;
		float blend = vec3(n_out216p0.xyz).x;
		if (base < 0.5) {
			n_out107p0.x = 2.0 * base * blend;
		} else {
			n_out107p0.x = 1.0 - 2.0 * (1.0 - blend) * (1.0 - base);
		}
	}
	{
		float base = vec3(n_out217p0.xyz).y;
		float blend = vec3(n_out216p0.xyz).y;
		if (base < 0.5) {
			n_out107p0.y = 2.0 * base * blend;
		} else {
			n_out107p0.y = 1.0 - 2.0 * (1.0 - blend) * (1.0 - base);
		}
	}
	{
		float base = vec3(n_out217p0.xyz).z;
		float blend = vec3(n_out216p0.xyz).z;
		if (base < 0.5) {
			n_out107p0.z = 2.0 * base * blend;
		} else {
			n_out107p0.z = 1.0 - 2.0 * (1.0 - blend) * (1.0 - base);
		}
	}
	float n_out107p1 = n_out107p0.r;


// FloatOp:113
	float n_in113p1 = 0.00000;
	float n_out113p0 = n_out107p1 - n_in113p1;


// FloatParameter:115
	float n_out115p0 = SpecularThreshold;


// Step:114
	float n_out114p0 = step(n_out113p0, n_out115p0);


// ColorParameter:117
	vec4 n_out117p0 = SpecularColour;


// VectorOp:116
	vec4 n_out116p0 = vec4(n_out114p0) * n_out117p0;


// VectorDecompose:123
	float n_out123p0 = n_out116p0.x;
	float n_out123p1 = n_out116p0.y;
	float n_out123p2 = n_out116p0.z;
	float n_out123p3 = n_out116p0.w;


// FloatFunc:126
	float n_out126p0 = ceil(n_out60p0);


// FloatOp:121
	float n_out121p0 = n_out123p3 * n_out126p0;


// VectorCompose:122
	vec4 n_out122p0 = vec4(n_out123p0, n_out123p1, n_out123p2, n_out121p0);


// Mix:124
	vec4 n_out124p0 = mix(n_out61p0, n_out122p0, vec4(n_out121p0));
	float n_out124p4 = n_out124p0.a;


// Mix:52
	vec4 n_out52p0 = mix(n_out49p0, n_out124p0, vec4(n_out124p4));


// ColorParameter:132
	vec4 n_out132p0 = FoamColour;


// VectorDecompose:173
	float n_out173p0 = n_out132p0.x;
	float n_out173p1 = n_out132p0.y;
	float n_out173p2 = n_out132p0.z;
	float n_out173p3 = n_out132p0.w;


// Input:221
	vec4 n_out221p0 = inputMask;


// Input:155
	float n_out155p0 = uTime;


// FloatParameter:172
	float n_out172p0 = FoamWaveSpeed;


// FloatOp:156
	float n_out156p0 = n_out155p0 * n_out172p0;


// VaryingGetter:142
	vec2 n_out142p0 = pixelWorld;


// FloatParameter:164
	float n_out164p0 = FoamNoiseScale;


// MultiplyAdd:186
	vec2 n_in186p2 = vec2(0.00000, 0.00000);
	vec2 n_out186p0 = (n_out142p0 * vec2(n_out164p0) + n_in186p2);


	vec4 n_out177p0;
// Texture2D:177
	n_out177p0 = texture(FoamNoiseTexture, n_out186p0);


// FloatParameter:163
	float n_out163p0 = FoamNoiseAmount;


// FloatOp:187
	float n_out187p0 = n_out177p0.x * n_out163p0;


// FloatOp:188
	float n_out188p0 = n_out156p0 * n_out187p0;


// Input:130
	vec4 n_out130p0 = inputMask;


// FloatParameter:182
	float n_out182p0 = FoamFrequency;


// VectorOp:183
	vec2 n_out183p0 = vec2(n_out130p0.xy) * vec2(n_out182p0);


// FloatOp:178
	float n_out178p0 = n_out188p0 - n_out183p0.x;


// FloatFunc:179
	float n_out179p0 = sin(n_out178p0);


	float n_out165p0;
// Remap:165
	float n_in165p1 = -1.00000;
	float n_in165p2 = 1.00000;
	float n_in165p3 = 0.10000;
	float n_in165p4 = 0.40000;
	{
		float inputRange = n_in165p2 - n_in165p1;
		float outputRange = n_in165p4 - n_in165p3;
		n_out165p0 = n_in165p3 + outputRange * ((n_out179p0 - n_in165p1) / inputRange);
	}


// FloatOp:181
	float n_out181p0 = n_out221p0.x - n_out165p0;


// Clamp:184
	float n_in184p1 = -0.00000;
	float n_in184p2 = 0.40000;
	float n_out184p0 = clamp(n_out181p0, n_in184p1, n_in184p2);


// FloatParameter:180
	float n_out180p0 = FoamQuantizeAmount;


// FloatOp:169
	float n_out169p0 = n_out184p0 * n_out180p0;


// FloatFunc:170
	float n_out170p0 = ceil(n_out169p0);


// FloatOp:171
	float n_out171p0 = n_out170p0 / n_out180p0;


// FloatOp:174
	float n_out174p0 = n_out171p0 * n_out173p3;


// VectorCompose:175
	vec4 n_out175p0 = vec4(n_out173p0, n_out173p1, n_out173p2, n_out174p0);
	float n_out175p4 = n_out175p0.a;


// Mix:136
	vec4 n_out136p0 = mix(n_out52p0, n_out175p0, vec4(n_out175p4));


// Input:55
	vec4 n_out55p0 = inputMask;


// VectorOp:56
	vec4 n_out56p0 = n_out136p0 * n_out55p0;
	float n_out56p4 = n_out56p0.a;


// Output:0
	finalColor.rgb = vec3(n_out136p0.xyz);
	finalColor.a = 1.0;


}
