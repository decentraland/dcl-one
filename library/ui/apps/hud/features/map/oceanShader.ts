// The sea around Genesis City, as a fragment shader over the map: a height map and a ground
// texture give the land and its shores, and noise-driven waves, highlights and foam move the
// water. It works in linear light, like the textures and colours it is given, and writes sRGB.
//
// The water model this follows credits its 'hash' and 'noise' functions to iq, under the Creative
// Commons Attribution-NonCommercial-ShareAlike 3.0 Unported License.

/** Side of the square the sea covers, map units, centred on the map's origin. */
export const OCEAN_SIZE = 24000

const srgbToLinear = (c: number): number => (c <= 0.04045 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4))
const linear = (r: number, g: number, b: number): string => `vec3(${[r, g, b].map((c) => srgbToLinear(c).toFixed(6)).join(', ')})`

export const OCEAN_VERTEX = `
attribute vec2 aPosition;
void main() {
  gl_Position = vec4(aPosition, 0.0, 1.0);
}
`

export const OCEAN_FRAGMENT = `
precision highp float;

uniform vec2 uCamera;      // camera centre, map units
uniform float uOrtho;      // half the height in view, map units
uniform vec2 uResolution;  // drawing buffer size, px
uniform float uTime;       // seconds
uniform sampler2D uHeight;
uniform sampler2D uGround;
uniform sampler2D uNoise;

const float OCEAN_SIZE = ${OCEAN_SIZE.toFixed(1)};
const float NOISE_TILING = 25.0;
// the ground textures cover the middle half of the sea, a touch off centre
const vec2 GROUND_SCALE = vec2(2.0, 2.0);
const vec2 GROUND_OFFSET = vec2(0.51, 0.49);
const float HEIGHT_CLAMP = 10.0;

const float WATER_LEVEL = 1.31 * 0.73;
const float COAST_TO_WATER_FADE_DEPTH = 0.672;
const float DEEP_WATER_FADE_DEPTH = 1.0 * 1.02 + COAST_TO_WATER_FADE_DEPTH;
const float HEIGHT_UV_MULTIPLIER = 17.2;
const float LARGE_WAVE_HEIGHT = 0.982;
const float LARGE_WAVE_SIZE = 3.44;
const float SMALL_WAVE_HEIGHT = 0.037;
const float SMALL_WAVE_SIZE = 0.497;
const float WATER_SOFT_LIGHT = 14.3;
const float WATER_GLOSSY_LIGHT = 103.0;
const float PARTICLE_AMOUNT = 68.8;
const vec3 WATER_COLOR = ${linear(0.23985049, 0.39756644, 0.7433963)};
const vec3 DEEP_WATER_COLOR = ${linear(0.16470586, 0.7058824, 0.50797063)};
const vec3 WATER_SPECULAR_COLOR = ${linear(0.74831605, 0.84731823, 0.85660374)};
const float INTENSITY_RED = ${srgbToLinear(0.7509433).toFixed(6)};
const vec3 LIGHT = vec3(0.5, 0.5, 3.0);

vec3 toLinear(vec3 c) {
  return mix(c / 12.92, pow((c + 0.055) / 1.055, vec3(2.4)), step(vec3(0.04045), c));
}
vec3 toSrgb(vec3 c) {
  c = clamp(c, 0.0, 1.0);
  return mix(c * 12.92, 1.055 * pow(c, vec3(1.0 / 2.4)) - 0.055, step(vec3(0.0031308), c));
}

float noise(vec2 p) {
  return toLinear(texture2D(uNoise, p / 256.0).rgb).x;
}

// the wave matrix applied to a column vector, and to a row vector
vec2 turn(vec2 p) {
  return vec2(0.72 * p.x - 1.60 * p.y, 1.60 * p.x + 0.72 * p.y);
}
vec2 turnBack(vec2 p) {
  return vec2(0.72 * p.x + 1.60 * p.y, -1.60 * p.x + 0.72 * p.y);
}

float heightMap(vec2 uv) {
  float f = 0.0;
  uv = uv * GROUND_SCALE - GROUND_OFFSET;
  if (uv.x > 0.0 && uv.x < 1.0 && uv.y > 0.0 && uv.y < 1.0) f = toLinear(texture2D(uHeight, uv).rgb).x;
  return clamp(f, 0.0, HEIGHT_CLAMP);
}

vec3 terrainMap(vec2 uv) {
  return toLinear(texture2D(uGround, uv * GROUND_SCALE - GROUND_OFFSET).rgb);
}

float waterMap(vec2 p, float height) {
  vec2 p2 = p * LARGE_WAVE_SIZE;
  vec2 shift1 = 0.001 * vec2(uTime * 320.0, uTime * 240.0);
  vec2 shift2 = 0.001 * vec2(uTime * 380.0, -uTime * 260.0);

  float f = 0.6000 * noise(p);
  f += 0.2500 * noise(turnBack(p));
  f += 0.1666 * noise(turnBack(turnBack(p)));
  float wave = sin(p2.x * 0.622 + p2.y * 0.622 + shift2.x * 4.269) * LARGE_WAVE_HEIGHT * f * height * height;

  p *= SMALL_WAVE_SIZE;
  f = 0.0;
  float amp = 1.0;
  float s = 0.5;
  for (int i = 0; i < 9; i++) {
    p = turn(p) * 0.947;
    f -= amp * abs(sin((noise(p + shift1 * s) - 0.5) * 2.0));
    amp = amp * 0.59;
    s *= -1.329;
  }
  return (wave + f) * SMALL_WAVE_HEIGHT;
}

float nautic(vec2 p) {
  p *= 18.0;
  float f = 0.0;
  float amp = 1.0;
  float s = 0.5;
  for (int i = 0; i < 3; i++) {
    p = turn(p) * 1.2;
    f += amp * abs(smoothstep(0.0, 1.0, noise(p + uTime * s)) - 0.5);
    amp = amp * 0.5;
    s *= -1.227;
  }
  return pow(abs(1.0 - f), 5.0);
}

float particles(vec2 p) {
  p *= 200.0;
  float f = 0.0;
  float amp = 1.0;
  float s = 1.5;
  for (int i = 0; i < 3; i++) {
    p = turn(p) * 1.2;
    f += amp * noise(p + uTime * s);
    amp = amp * 0.5;
    s *= -1.227;
  }
  return pow(f * 0.35, 7.0) * PARTICLE_AMOUNT;
}

void main() {
  vec2 map = uCamera + (gl_FragCoord.xy - uResolution * 0.5) * (2.0 * uOrtho / uResolution.y);
  vec2 uv = map / OCEAN_SIZE + 0.5;
  vec2 uvNoise = uv * NOISE_TILING;

  float height = heightMap(uv);
  vec3 col = vec3(0.0);

  float waveheight = clamp(WATER_LEVEL * 3.0 - 1.5, 0.0, 1.0);
  float level = WATER_LEVEL + 0.2 * waterMap(uvNoise * 15.0 + vec2(uTime * 0.1), waveheight);
  if (height > level) {
    col = terrainMap(uv);
  } else {
    vec2 dif = vec2(0.0, 0.01);
    vec2 pos = uv * HEIGHT_UV_MULTIPLIER + vec2(uTime * 0.01);
    float h1 = waterMap(pos - dif, waveheight);
    float h2 = waterMap(pos + dif, waveheight);
    float h3 = waterMap(pos - dif.yx, waveheight);
    float h4 = waterMap(pos + dif.yx, waveheight);
    vec3 normwater = normalize(vec3(h3 - h4, h1 - h2, 0.125));
    vec2 lit = uv + normwater.xy * 0.002 * (level - height);

    col = terrainMap(uv);

    float coastfade = clamp((level - height) / COAST_TO_WATER_FADE_DEPTH, 0.0, 1.0);
    float coastfade2 = clamp((level - height) / DEEP_WATER_FADE_DEPTH, 0.0, 1.0);

    float intensity = col.r * INTENSITY_RED;
    vec3 water = mix(WATER_COLOR * intensity, DEEP_WATER_COLOR, smoothstep(0.0, 1.0, coastfade2));

    vec3 rd = normalize(LIGHT - vec3(lit, WATER_LEVEL));
    float grad = dot(normwater, rd);
    float specular = pow(abs(grad), WATER_SOFT_LIGHT);
    float specular2 = pow(abs(grad), WATER_GLOSSY_LIGHT);
    float gradpos = dot(vec3(0.0, 0.0, 1.0), rd);
    float specular1 = smoothstep(0.0, 1.0, pow(abs(gradpos), 5.0));
    water *= 3.2;
    water += ((grad - 1.0) * 0.5 + specular) * 0.25;
    water /= (1.0 + specular1 * 1.25);
    water += specular2 * WATER_SPECULAR_COLOR;
    water += coastfade * (1.0 - coastfade2) * (vec3(0.5, 0.6, 0.7) * nautic(uvNoise) + vec3(1.0) * particles(uvNoise));

    col = mix(col, water, coastfade);
  }

  gl_FragColor = vec4(toSrgb(col), 1.0);
}
`
