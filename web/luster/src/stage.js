// What a badge is drawn in, whichever view shows it: the studio the engine
// lights every badge with, drawn by three.js. One WebGL context serves the
// page; each view's frame is drawn there and copied onto the view's own
// canvas, as a browser allows only a few contexts a page and loses the
// oldest past them.
import * as THREE from 'three';
import init, * as engine from '../engine/luster_wasm.js';
import { ask, compile } from './engine.js';

let here;

/** The engine on this thread, for what a view asks of it as it draws: a few
 *  numbers, and a turn's coast every frame. Nothing here mints. */
export function local() {
  return (here ??= compile()
    .then((module) => init({ module_or_path: module }))
    .then(() => engine)
    .catch((e) => {
      here = undefined;
      throw e;
    }));
}

/** Copies a colour out of the engine and lets go of its own. */
export function rgba(made) {
  const out = { r: made.r, g: made.g, b: made.b, a: made.a };
  made.free();
  return out;
}

const linear = (c) => (c <= 0.04045 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4));

/** Edge of the irradiance cube's faces: a diffuse surface's light changes
 *  slowly with its direction. */
const IRRADIANCE_SIZE = 16;

let shared;

/** The studio, set up once for the page; a failure is not kept. */
export function studio() {
  return (shared ??= Studio.make().catch((e) => {
    shared = undefined;
    throw e;
  }));
}

const VERTEX = /* glsl */ `
attribute vec4 tangent;
varying vec3 vWorldPosition;
varying vec3 vNormal;
varying vec4 vTangent;
varying vec2 vUv;

void main() {
  vec4 world = modelMatrix * vec4(position, 1.0);
  vWorldPosition = world.xyz;
  // The badge is only turned, never scaled.
  mat3 turn = mat3(modelMatrix);
  vNormal = turn * normal;
  vTangent = vec4(turn * tangent.xyz, tangent.w);
  vUv = uv;
  gl_Position = projectionMatrix * viewMatrix * world;
}
`;

// The badge lit by the studio's panorama alone, as the Flutter plugin's
// shader (flutter/luster/shaders/luster_badge.frag) lights it: the split-sum
// specular with multiple-scattering energy, the diffuse light, and the
// specular anti-aliasing of a normal-mapped surface. The diffuse light is the
// engine's own irradiance rather than spherical harmonics, and the
// prefiltered panorama three.js's, read as flutter_scene reads its own.
const FRAGMENT = /* glsl */ `
#include <common>
#include <cube_uv_reflection_fragment>

uniform sampler2D map;
uniform sampler2D normalMap;
uniform samplerCube irradiance;
uniform sampler2D envMap;
uniform sampler2D dfgLUT;
// Linear base colour, and alpha.
uniform vec4 baseColor;
uniform vec3 dielectricF0;
// Metallic, perceptual roughness, normal scale, whether there is a normal map.
uniform vec4 surface;
// Environment intensity, specular anti-aliasing variance and threshold.
uniform vec3 environment;
// Turns a direction in the scene into the one three.js's panorama is
// sampled by.
uniform mat3 panoramaAxes;

varying vec3 vWorldPosition;
varying vec3 vNormal;
varying vec4 vTangent;
varying vec2 vUv;

const float kMinRoughness = 0.045;

// flutter_scene prefilters its panorama at eight roughnesses, 0 to 1, and
// blends the two either side; a gloss between them takes some of the
// blurrier one, and RealityKit's reflections are softer still. Sampled the
// same way, a roughness 1.2 times the material's brings three.js's
// reflections to LusterShot's, the reverse's sandblast most of all.
const float kBands = 8.0;
const float kBlur = 1.2;

vec3 prefilteredRadiance(vec3 direction, float roughness) {
  float band = clamp(roughness * kBlur, 0.0, 1.0) * (kBands - 1.0);
  float below = floor(band);
  float above = min(below + 1.0, kBands - 1.0);
  return mix(
    textureCubeUV(envMap, direction, below / (kBands - 1.0)).rgb,
    textureCubeUV(envMap, direction, above / (kBands - 1.0)).rgb,
    band - below);
}

float antiAliasedRoughness(vec3 normal, float roughness) {
  vec3 dx = dFdx(normal);
  vec3 dy = dFdy(normal);
  float variance = environment.y * max(dot(dx, dx), dot(dy, dy));
  float kernel = min(2.0 * variance, environment.z);
  return sqrt(clamp(roughness * roughness + kernel, kMinRoughness * kMinRoughness, 1.0));
}

vec3 fresnelSchlickRoughness(float cosTheta, vec3 f0, float roughness) {
  return f0 + (max(vec3(1.0 - roughness), f0) - f0) * pow(clamp(1.0 - cosTheta, 0.0, 1.0), 5.0);
}

void main() {
  vec4 texel = texture2D(map, vUv);
  vec3 albedo = texel.rgb * baseColor.rgb;
  float alpha = texel.a * baseColor.a;

  vec3 geometric = normalize(vNormal);
  vec3 normal = geometric;
  if (surface.w > 0.5) {
    vec3 t = normalize(vTangent.xyz - geometric * dot(geometric, vTangent.xyz));
    vec3 b = cross(geometric, t) * vTangent.w;
    vec3 mapped = texture2D(normalMap, vUv).xyz * 2.0 - 1.0;
    mapped.xy *= surface.z;
    normal = normalize(mat3(t, b, geometric) * mapped);
  }
  float metallic = clamp(surface.x, 0.0, 1.0);
  float roughness = antiAliasedRoughness(normal, clamp(surface.y, kMinRoughness, 1.0));

  vec3 view = normalize(cameraPosition - vWorldPosition);
  vec3 reflectance = mix(dielectricF0, albedo, metallic);
  // The Fresnel and the split-sum energy take the geometric normal, the
  // reflection the perturbed one.
  float nDotV = max(dot(geometric, view), 0.0);
  vec3 reflected = reflect(-view, normal);
  vec3 kS = fresnelSchlickRoughness(nDotV, reflectance, roughness);

  vec3 diffuse = textureCube(irradiance, normal).rgb * environment.x;
  vec3 prefiltered = prefilteredRadiance(panoramaAxes * reflected, roughness) * environment.x;

  vec2 fab = texture2D(dfgLUT, vec2(roughness, nDotV)).rg;
  vec3 fssEss = kS * fab.x + fab.y;
  float ems = 1.0 - (fab.x + fab.y);
  vec3 fAvg = reflectance + (1.0 - reflectance) / 21.0;
  vec3 fmsEms = ems * fssEss * fAvg / (1.0 - fAvg * ems);
  vec3 kD = albedo * (1.0 - metallic) * (1.0 - fssEss + fmsEms);

  vec3 color = fssEss * prefiltered + (fmsEms + kD) * diffuse;
  gl_FragColor = vec4(color, 1.0) * alpha;
}
`;

// What a frame drawn in linear light shows once it is exposed, clamped and
// read through RealityKit's tone curve: what every platform shows.
const TONE_VERTEX = /* glsl */ `
varying vec2 vUv;
void main() {
  vUv = position.xy * 0.5 + 0.5;
  gl_Position = vec4(position.xy, 0.0, 1.0);
}
`;

const TONE_FRAGMENT = /* glsl */ `
uniform sampler2D frame;
uniform sampler3D tone;
// What of the frame's texture the view's frame takes up.
uniform vec2 extent;
uniform float exposure;
uniform float toneSize;
varying vec2 vUv;

void main() {
  vec4 hdr = texture2D(frame, vUv * extent);
  vec3 color = hdr.a > 0.0 ? hdr.rgb / hdr.a : vec3(0.0);
  vec3 mapped = sRGBTransferOETF(vec4(clamp(color * exposure, 0.0, 1.0), 1.0)).rgb;
  vec3 shown = texture(tone, mapped * ((toneSize - 1.0) / toneSize) + 0.5 / toneSize).rgb;
  gl_FragColor = vec4(shown * hdr.a, hdr.a);
}
`;

/** The engine's panorama is laid out by longitude atan2(x, -z) in its
 *  right-handed space, three.js's by atan2(z, x): a quarter turn about y. */
const PANORAMA_AXES = new THREE.Matrix3().set(0, 0, -1, 0, 1, 0, 1, 0, 0);

class Studio {
  static async make() {
    const [here, images] = await Promise.all([local(), ask('studio', { irradianceSize: IRRADIANCE_SIZE })]);
    const canvas = document.createElement('canvas');
    canvas.width = canvas.height = 1;
    const renderer = new THREE.WebGLRenderer({
      canvas,
      alpha: true,
      premultipliedAlpha: true,
      antialias: false,
      depth: false,
      powerPreference: 'high-performance',
    });
    try {
      // The frame is drawn in floating point, multisampled.
      if (!renderer.extensions.has('EXT_color_buffer_float') && !renderer.extensions.has('EXT_color_buffer_half_float')) {
        throw new Error('this browser cannot draw into floating-point buffers');
      }
      return new Studio(here, images, renderer);
    } catch (e) {
      // A failed studio leaves no context behind: the page has few.
      renderer.dispose();
      renderer.forceContextLoss();
      throw e;
    }
  }

  constructor(here, { panorama, irradiance, sandblast }, renderer) {
    this.engine = here;
    this.renderer = renderer;
    this.canvas = renderer.domElement;
    renderer.setPixelRatio(1);
    renderer.setClearColor(0x000000, 0);
    renderer.toneMapping = THREE.NoToneMapping;

    const calibration = here.sceneCalibration();
    this.exposure = calibration.exposure;
    this.lampUnit = calibration.lampUnit;
    calibration.free();
    const camera = here.studioCamera();
    this.distance = camera.distance;
    camera.free();
    const handling = here.handling();
    this.handling = {
      spinPerPoint: handling.spinPerPoint,
      tiltPerPoint: handling.tiltPerPoint,
      tiltLimit: handling.tiltLimit,
      restingTilt: handling.restingTilt,
      restingSpin: handling.restingSpin,
    };
    handling.free();
    this.lights = here.studioLights().map((l) => {
      const out = { role: l.role, color: rgba(l.color), direction: l.direction };
      l.free();
      return out;
    });
    this.metals = { gold: rgba(here.defaultGold()), silver: rgba(here.silver()), copper: rgba(here.copper()) };

    this.panorama = equirectangular(panorama);
    this.irradiance = cube(irradiance);
    this.grit = new THREE.DataTexture(sandblast.rgba, sandblast.width, sandblast.height);
    this.grit.wrapS = this.grit.wrapT = THREE.RepeatWrapping;
    this.grit.minFilter = THREE.LinearMipmapLinearFilter;
    this.grit.magFilter = THREE.LinearFilter;
    this.grit.generateMipmaps = true;
    this.grit.needsUpdate = true;
    this.white = new THREE.DataTexture(new Uint8Array([255, 255, 255, 255]), 1, 1);
    this.white.needsUpdate = true;

    const tone = here.realitykitTone();
    this.tone = toneTexture(tone);
    this.toneSize = tone.size;
    tone.free();

    // Drawn at the largest any view has asked for, and grown, never shrunk:
    // a view smaller draws into a corner of it.
    this.frame = new THREE.WebGLRenderTarget(1, 1, {
      type: THREE.HalfFloatType,
      samples: 4,
      depthBuffer: true,
    });
    this.toneMaterial = new THREE.ShaderMaterial({
      vertexShader: TONE_VERTEX,
      fragmentShader: TONE_FRAGMENT,
      uniforms: {
        frame: { value: this.frame.texture },
        tone: { value: this.tone },
        extent: { value: new THREE.Vector2(1, 1) },
        exposure: { value: this.exposure },
        toneSize: { value: this.toneSize },
      },
      depthTest: false,
      depthWrite: false,
    });
    const triangle = new THREE.BufferGeometry();
    triangle.setAttribute('position', new THREE.Float32BufferAttribute([-1, -1, 0, 3, -1, 0, -1, 3, 0], 3));
    this.toneScene = new THREE.Scene();
    const toned = new THREE.Mesh(triangle, this.toneMaterial);
    toned.frustumCulled = false;
    this.toneScene.add(toned);
    this.toneCamera = new THREE.Camera();

    this.prefilter();
    this.finishes = new Map();
    this.presets = new Map();
    this.lamps = new Map();
    this.badges = new Map();

    // A lost context takes every texture and target with it; three.js makes
    // them again when they are next used, but the prefiltered panorama is
    // drawn, not uploaded, and is drawn again.
    this.lost = false;
    this.canvas.addEventListener('webglcontextlost', (e) => {
      e.preventDefault();
      this.lost = true;
    });
    this.canvas.addEventListener('webglcontextrestored', () => {
      this.lost = false;
      this.prefilter();
      for (const view of this.views) view.lost();
    });
    /** Every stage drawing here, told when the context comes back. */
    this.views = new Set();
  }

  prefilter() {
    const pmrem = new THREE.PMREMGenerator(this.renderer);
    this.environment?.dispose();
    this.environment = pmrem.fromEquirectangular(this.panorama);
    pmrem.dispose();
    const height = this.environment.texture.image.height;
    const maxMip = Math.log2(height) - 2;
    this.cubeUV = {
      ENVMAP_TYPE_CUBE_UV: '',
      CUBEUV_TEXEL_WIDTH: 1 / (3 * Math.max(Math.pow(2, maxMip), 7 * 16)),
      CUBEUV_TEXEL_HEIGHT: 1 / height,
      CUBEUV_MAX_MIP: `${maxMip}.0`,
    };
  }

  /** How a material of `role` is finished under `lighting`. */
  finish(role, lighting) {
    const key = `${role} ${lighting}`;
    let finish = this.finishes.get(key);
    if (!finish) {
      const f = this.engine.finish(role, lighting);
      finish = { metallic: f.metallic, roughness: f.roughness, specular: f.specular, plated: f.plated };
      f.free();
      this.finishes.set(key, finish);
    }
    return finish;
  }

  preset(lighting) {
    let preset = this.presets.get(lighting);
    if (!preset) {
      const p = this.engine.lightingPreset(lighting);
      preset = { environment: p.environment, sceneLux: p.sceneLux };
      p.free();
      this.presets.set(lighting, preset);
    }
    return preset;
  }

  lamp(role, lighting) {
    const key = `${role} ${lighting}`;
    let worth = this.lamps.get(key);
    if (worth === undefined) {
      worth = this.engine.lamp(role, lighting);
      this.lamps.set(key, worth);
    }
    return worth;
  }

  /** The badge's meshes and textures on the GPU, shared by every view
   *  showing it until the last lets go. Each view is given a badge of its
   *  own, but those of one mint share their meshes, and are known by them. */
  hold(badge) {
    let held = this.badges.get(badge.submeshes);
    if (!held) {
      held = { users: 0, geometries: badge.submeshes.map(geometry), sheets: badge.textures.map(sheet) };
      this.badges.set(badge.submeshes, held);
    }
    held.users++;
    return held;
  }

  release(badge) {
    const held = this.badges.get(badge.submeshes);
    if (!held || --held.users > 0) return;
    this.badges.delete(badge.submeshes);
    for (const g of held.geometries) g.dispose();
    for (const s of held.sheets) s.dispose();
  }

  /** The material for one of the badge's, plated in `metal` (sRGB) and lit
   *  for `lighting`. */
  material(m, sheets, metal, lighting) {
    const finish = this.finish(m.role, lighting);
    const color = finish.plated ? metal : m.color;
    const base = new THREE.Color(linear(color.r), linear(color.g), linear(color.b));
    const map = m.texture !== undefined && m.texture < sheets.length ? sheets[m.texture] : null;
    const normalMap = m.role === 'goldBack' ? this.grit : null;
    if (this.lit(lighting)) {
      // The lamps' lighting: three.js's own material, as flutter_scene's
      // standard one lights it. The badge's shader is the panorama's alone.
      return new THREE.MeshPhysicalMaterial({
        color: base,
        metalness: finish.metallic,
        roughness: finish.roughness,
        specularIntensity: finish.specular,
        map,
        normalMap,
      });
    }
    const f0 = Math.min(Math.max(0.04 * finish.specular, 0), 1);
    return new THREE.ShaderMaterial({
      vertexShader: VERTEX,
      fragmentShader: FRAGMENT,
      defines: this.cubeUV,
      uniforms: {
        map: { value: map ?? this.white },
        normalMap: { value: normalMap ?? this.white },
        irradiance: { value: this.irradiance },
        envMap: { value: this.environment.texture },
        // three.js fills it in.
        dfgLUT: { value: null },
        baseColor: { value: new THREE.Vector4(base.r, base.g, base.b, 1) },
        dielectricF0: { value: new THREE.Vector3(f0, f0, f0) },
        surface: { value: new THREE.Vector4(finish.metallic, finish.roughness, 1, normalMap ? 1 : 0) },
        environment: { value: new THREE.Vector3(this.preset(lighting).environment, 0.15, 0.2) },
        panoramaAxes: { value: PANORAMA_AXES },
      },
    });
  }

  /** Whether `lighting` lights any lamp. */
  lit(lighting) {
    return this.lights.some((light) => this.lamp(light.role, lighting) > 0);
  }

  /** Gives back the frame's memory once no view is left to draw. */
  shrink() {
    this.frame.setSize(1, 1);
    this.renderer.setSize(1, 1, false);
  }

  /** The lamps `lighting` lights the studio with, in three.js's units. */
  lampsFor(lighting) {
    const preset = this.preset(lighting);
    const lamps = [];
    for (const light of this.lights) {
      const worth = this.lamp(light.role, lighting);
      if (worth <= 0) continue;
      const lamp = new THREE.DirectionalLight(
        new THREE.Color(light.color.r, light.color.g, light.color.b),
        worth * preset.sceneLux * this.lampUnit,
      );
      // Shining down `direction`: from the far side of the badge's centre.
      const [x, y, z] = light.direction;
      lamp.position.set(-x, -y, -z);
      lamps.push(lamp);
    }
    return lamps;
  }

  /**
   * Draws `scene` from `camera` into a `width` by `height` frame and copies it
   * onto `context`, a 2D context of that size.
   */
  draw(scene, camera, width, height, context) {
    if (this.lost) return false;
    const renderer = this.renderer;
    const frame = this.frame;
    if (frame.width < width || frame.height < height) {
      frame.setSize(Math.max(frame.width, width), Math.max(frame.height, height));
    }
    if (this.canvas.width < width || this.canvas.height < height) {
      renderer.setSize(Math.max(this.canvas.width, width), Math.max(this.canvas.height, height), false);
    }
    frame.viewport.set(0, 0, width, height);
    frame.scissor.set(0, 0, width, height);
    frame.scissorTest = true;
    renderer.setRenderTarget(frame);
    renderer.clear();
    renderer.render(scene, camera);

    this.toneMaterial.uniforms.extent.value.set(width / frame.width, height / frame.height);
    renderer.setRenderTarget(null);
    renderer.setViewport(0, 0, width, height);
    renderer.setScissor(0, 0, width, height);
    renderer.setScissorTest(true);
    renderer.clear();
    renderer.render(this.toneScene, this.toneCamera);
    renderer.setScissorTest(false);

    // The frame is the canvas's bottom-left corner: WebGL's rows run up.
    context.clearRect(0, 0, width, height);
    context.drawImage(this.canvas, 0, this.canvas.height - height, width, height, 0, 0, width, height);
    return true;
  }
}

/** The engine's panorama, rows top to bottom, for three.js, which reads an
 *  equirectangular image with its first row at the bottom. */
function equirectangular({ width, height, rgba }) {
  const row = width * 4;
  const flipped = new Uint8Array(rgba.length);
  for (let y = 0; y < height; y++) {
    flipped.set(rgba.subarray(y * row, (y + 1) * row), (height - 1 - y) * row);
  }
  const texture = new THREE.DataTexture(flipped, width, height);
  texture.colorSpace = THREE.SRGBColorSpace;
  texture.mapping = THREE.EquirectangularReflectionMapping;
  // Longitude wraps; without it the seam behind shows as a line.
  texture.wrapS = THREE.RepeatWrapping;
  texture.magFilter = THREE.LinearFilter;
  texture.minFilter = THREE.LinearFilter;
  texture.needsUpdate = true;
  return texture;
}

/** Six faces stacked, +X to -Z, in WebGL's own orientation. */
function cube({ width, rgba }) {
  const face = width * width * 4;
  const faces = Array.from({ length: 6 }, (_, i) => {
    const t = new THREE.DataTexture(rgba.subarray(i * face, (i + 1) * face), width, width);
    return t;
  });
  const texture = new THREE.CubeTexture(faces);
  texture.colorSpace = THREE.SRGBColorSpace;
  texture.magFilter = THREE.LinearFilter;
  texture.minFilter = THREE.LinearFilter;
  texture.generateMipmaps = false;
  texture.needsUpdate = true;
  return texture;
}

function toneTexture({ size, rgb }) {
  const data = new Uint8Array(size * size * size * 4);
  for (let i = 0, j = 0; i < rgb.length; i += 3, j += 4) {
    data[j] = rgb[i];
    data[j + 1] = rgb[i + 1];
    data[j + 2] = rgb[i + 2];
    data[j + 3] = 255;
  }
  const texture = new THREE.Data3DTexture(data, size, size, size);
  texture.magFilter = THREE.LinearFilter;
  texture.minFilter = THREE.LinearFilter;
  texture.unpackAlignment = 1;
  texture.needsUpdate = true;
  return texture;
}

function geometry(s) {
  const g = new THREE.BufferGeometry();
  const vertices = new THREE.InterleavedBuffer(s.vertices, 12);
  g.setAttribute('position', new THREE.InterleavedBufferAttribute(vertices, 3, 0));
  g.setAttribute('normal', new THREE.InterleavedBufferAttribute(vertices, 3, 3));
  g.setAttribute('uv', new THREE.InterleavedBufferAttribute(vertices, 2, 6));
  g.setAttribute('tangent', new THREE.InterleavedBufferAttribute(vertices, 4, 8));
  g.setIndex(new THREE.BufferAttribute(s.indices, 1));
  g.boundingBox = new THREE.Box3(new THREE.Vector3(...s.min), new THREE.Vector3(...s.max));
  g.boundingSphere = g.boundingBox.getBoundingSphere(new THREE.Sphere());
  return g;
}

/** A sheet with the mip chain the engine made for it, so that every
 *  renderer filters the same levels. */
function sheet({ width, height, levels }) {
  const mipmaps = levels.map((data, i) => ({
    data,
    width: Math.max(1, width >> i),
    height: Math.max(1, height >> i),
  }));
  const texture = new THREE.DataTexture(levels[0], width, height);
  texture.mipmaps = mipmaps;
  texture.colorSpace = THREE.SRGBColorSpace;
  texture.minFilter = THREE.LinearMipmapLinearFilter;
  texture.magFilter = THREE.LinearFilter;
  texture.generateMipmaps = false;
  texture.needsUpdate = true;
  return texture;
}

/** One view's badge in the studio: its own scene and camera, drawn in the
 *  page's context. */
export class Stage {
  constructor(studio) {
    this.studio = studio;
    this.scene = new THREE.Scene();
    this.pivot = new THREE.Group();
    this.scene.add(this.pivot);
    this.camera = new THREE.PerspectiveCamera(26, 1, 0.1, 100);
    this.camera.position.set(0, 0, studio.distance);
    this.camera.lookAt(0, 0, 0);
    this.lamps = [];
    this.badge = null;
    this.held = null;
    this.meshes = [];
    this.wide = 1;
    this.tall = 1;
    this.tilt = studio.handling.restingTilt;
    this.spin = studio.handling.restingSpin;
    this.onLost = null;
    studio.views.add(this);
  }

  /** Puts `badge` on stage, plated and lit as `appearance` says; null clears
   *  the stage. */
  show(badge, appearance) {
    if (badge === this.badge) {
      this.plate(appearance);
      return;
    }
    // Held before the last is let go: another badge of the same mint keeps
    // its meshes on the GPU.
    const held = badge ? this.studio.hold(badge) : null;
    this.clear();
    if (!badge) return;
    this.badge = badge;
    this.held = held;
    this.wide = 0;
    this.tall = 0;
    for (const [i, s] of badge.submeshes.entries()) {
      const mesh = new THREE.Mesh(this.held.geometries[i]);
      mesh.name = s.name;
      this.meshes.push(mesh);
      this.pivot.add(mesh);
      this.wide = Math.max(this.wide, Math.abs(s.min[0]) * 2, Math.abs(s.max[0]) * 2);
      this.tall = Math.max(this.tall, Math.abs(s.min[1]) * 2, Math.abs(s.max[1]) * 2);
    }
    this.appearance = null;
    this.plate(appearance);
    this.pose(this.tilt, this.spin);
  }

  clear() {
    for (const mesh of this.meshes) {
      mesh.material?.dispose();
      this.pivot.remove(mesh);
    }
    this.meshes = [];
    if (this.badge) this.studio.release(this.badge);
    this.badge = null;
    this.held = null;
  }

  /** Plates and lights the badge as `appearance` says: `metal` sRGB, and
   *  `lighting` 'showcase' or 'off'. */
  plate(appearance) {
    const same =
      this.appearance &&
      this.appearance.lighting === appearance.lighting &&
      sameColor(this.appearance.metal, appearance.metal);
    this.appearance = appearance;
    if (same) return;
    for (const lamp of this.lamps) this.scene.remove(lamp);
    this.lamps = this.studio.lampsFor(appearance.lighting);
    for (const lamp of this.lamps) this.scene.add(lamp);
    if (!this.badge) return;
    for (const [i, s] of this.badge.submeshes.entries()) {
      const mesh = this.meshes[i];
      mesh.material?.dispose();
      mesh.material = this.studio.material(
        this.badge.materials[s.material],
        this.held.sheets,
        appearance.metal,
        appearance.lighting,
      );
    }
  }

  /** Turns the badge to `tilt` (about x) and `spin` (about y), radians. */
  pose(tilt, spin) {
    this.tilt = tilt;
    this.spin = spin;
    this.pivot.quaternion
      .setFromAxisAngle(X, tilt)
      .multiply(new THREE.Quaternion().setFromAxisAngle(Y, spin));
  }

  /** Draws the stage into `context`, `width` by `height` device pixels. */
  draw(context, width, height) {
    const aspect = width / Math.max(height, 1);
    this.camera.fov = this.studio.engine.fieldOfView(this.wide, this.tall, aspect);
    this.camera.aspect = aspect;
    this.camera.updateProjectionMatrix();
    return this.studio.draw(this.scene, this.camera, width, height, context);
  }

  /** The context came back: the materials take the panorama prefiltered
   *  again, and the view draws. */
  lost() {
    const appearance = this.appearance;
    this.appearance = null;
    if (appearance) this.plate(appearance);
    this.onLost?.();
  }

  dispose() {
    this.clear();
    for (const lamp of this.lamps) this.scene.remove(lamp);
    this.lamps = [];
    this.studio.views.delete(this);
    if (this.studio.views.size === 0) this.studio.shrink();
  }
}

const X = new THREE.Vector3(1, 0, 0);
const Y = new THREE.Vector3(0, 1, 0);

function sameColor(a, b) {
  return a.r === b.r && a.g === b.g && a.b === b.b && a.a === b.a;
}
