// The engine, in a worker of its own: a mint runs on this thread to the end.
// The badges it strikes are kept here, by an id, until the page frees them.
import init, * as engine from '../engine/luster_wasm.js';

const kept = new Map();
let serial = 0;

const ops = {
  async init({ module }) {
    await init({ module_or_path: module });
  },

  mint({ svg, options }) {
    const badge = engine.mint(svg, options);
    const id = ++serial;
    kept.set(id, badge);
    const transfer = [];
    const submeshes = badge.submeshes().map((s) => {
      const out = {
        name: s.name,
        material: s.material,
        vertices: s.vertices,
        indices: s.indices,
        vertexCount: s.vertexCount,
        min: s.min,
        max: s.max,
      };
      s.free();
      transfer.push(out.vertices.buffer, out.indices.buffer);
      return out;
    });
    const materials = badge.materials().map((m) => {
      const color = m.color;
      const out = {
        name: m.name,
        role: m.role,
        color: { r: color.r, g: color.g, b: color.b, a: color.a },
        metallic: m.metallic,
        roughness: m.roughness,
        texture: m.texture,
      };
      color.free();
      m.free();
      return out;
    });
    const textures = badge.textures().map((t) => {
      const levels = Array.from({ length: t.levelCount }, (_, i) => t.level(i));
      const out = { width: t.width, height: t.height, levels };
      t.free();
      transfer.push(...levels.map((l) => l.buffer));
      return out;
    });
    return [{ id, badge: { designKey: badge.designKey, submeshes, materials, textures } }, transfer];
  },

  glb({ id, scale, metal }) {
    const badge = kept.get(id);
    if (!badge) throw { kind: 'engineFailure', message: 'the badge is no longer held', gone: true };
    const color = metal ? new engine.Rgba(metal.r, metal.g, metal.b, metal.a) : engine.defaultGold();
    try {
      const glb = badge.glb(scale, color);
      return [glb, [glb.buffer]];
    } finally {
      color.free();
    }
  },

  free({ id }) {
    kept.get(id)?.free();
    kept.delete(id);
  },

  version() {
    return engine.version();
  },

  metals() {
    const read = (make) => {
      const c = make();
      const out = { r: c.r, g: c.g, b: c.b, a: c.a };
      c.free();
      return out;
    };
    return { gold: read(engine.defaultGold), silver: read(engine.silver), copper: read(engine.copper) };
  },
};

self.onmessage = async ({ data }) => {
  const { call, op } = data;
  try {
    const answer = await ops[op](data);
    const [result, transfer] = Array.isArray(answer) ? answer : [answer, []];
    self.postMessage({ call, result }, transfer);
  } catch (e) {
    self.postMessage({ call, error: failure(e) });
  }
};

/** What went wrong, as the page reads it. */
function failure(e) {
  if (e?.name === 'LusterError') return { kind: e.kind, message: e.message };
  // The engine trapped: its message is on the console, and the module is
  // not to be called again.
  if (e instanceof WebAssembly.RuntimeError) return { kind: 'engineFailure', message: e.message, trapped: true };
  if (e?.kind) return e;
  return { kind: 'engineFailure', message: String(e?.message ?? e) };
}
