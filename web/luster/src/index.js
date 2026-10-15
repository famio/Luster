// Entry points to the engine, for the web: badges struck in workers, off the
// thread that draws.
import { Engine, Gone, ask as askHelper, compile, endHelper } from './engine.js';
import { LusterError, engineError } from './errors.js';
import { Mints } from './mints.js';

export { LusterError };

const MAXIMUM_BYTES = 20 << 20;

let mints;
const page = () => (mints ??= new Mints({ spawn: () => new Engine() }));

async function ask(op) {
  try {
    return await askHelper(op);
  } catch (e) {
    throw engineError(e);
  }
}

export const Luster = {
  /** Larger documents are refused: before they are fetched in full when the
   *  server says how large they are. */
  get maximumBytes() {
    return MAXIMUM_BYTES;
  },

  /**
   * What a URL source is fetched with: `fetch` unless the page sets another.
   * @type {(url: URL, signal?: AbortSignal) => Promise<Uint8Array | ArrayBuffer>}
   */
  loader: fetchBytes,

  /**
   * Fetches and compiles the engine. A mint does it itself; calling it early
   * gets it done before the first badge is wanted. It is done once, and tried
   * again after a failure.
   */
  init() {
    return compile().then(() => undefined);
  },

  /** The engine's version. */
  version() {
    return ask('version');
  },

  /** The engine's metals: its pale gold, silver and copper. */
  metals() {
    return ask('metals');
  },

  /**
   * Mints a badge from an SVG: its text, its bytes, or a URL fetched with
   * `Luster.loader`.
   *
   * At most two mints run at once across the page; the rest wait their turn.
   * Recent badges are kept by their bytes and options, so asking again is
   * immediate, and asking for one already being minted waits for that mint
   * rather than starting another. An aborted `signal` rejects with its
   * reason, and the mint stops once no one else is waiting for it, as
   * `Luster.close()` stops it with an `AbortError`. Fails with a `LusterError`
   * when the document, its source or the engine fails.
   *
   * @param {string | URL | Uint8Array | ArrayBuffer} source
   * @param {{ withoutHiddenFaces?: boolean, metalLines?: boolean, signal?: AbortSignal }} [options]
   */
  async mint(source, { withoutHiddenFaces = false, metalLines = false, signal } = {}) {
    const svg = await load(source, signal);
    const options = { withoutHiddenFaces: withoutHiddenFaces === true, metalLines: metalLines === true };
    return new LusterBadge(await strike(svg, options, signal));
  },

  /**
   * Ends the workers, letting go of their memory, which a module never gives
   * back while it lives: a mint leaves its worker holding what the largest
   * badge it struck took. Mints under way reject; badges already made keep
   * their meshes, and their GLB mints them again.
   */
  close() {
    mints?.close();
    mints = undefined;
    endHelper();
  },
};

async function strike(svg, options, signal) {
  try {
    return await page().request(svg, options, signal);
  } catch (e) {
    throw signal?.aborted && e === signal.reason ? e : engineError(e);
  }
}

/** A struck badge: what a view shows, and the bytes to send one somewhere else. */
export class LusterBadge {
  #kept;
  #svg;
  #options;

  constructor(kept) {
    this.#kept = kept;
    // The mints' own copy: the caller's bytes may change after.
    this.#svg = kept.svg;
    this.#options = kept.options;
    const { designKey, submeshes, materials, textures } = kept.badge;
    /** Identifies the badge: the document and the options it was struck with. */
    this.designKey = designKey;
    /** Shared with everyone given the same badge: read them, change none. */
    this.submeshes = submeshes;
    this.materials = materials;
    this.textures = textures;
    Object.freeze(this);
  }

  /**
   * The badge as a glTF binary: meshes, materials and textures in one file,
   * its metal plated in `metal` (the engine's pale gold unless given), `scale`
   * units (metres, in glTF) across. The same badge always writes the same
   * bytes. An aborted `signal` rejects with its reason.
   *
   * @param {{ scale?: number, metal?: { r: number, g: number, b: number, a: number }, signal?: AbortSignal }} [options]
   * @returns {Promise<Uint8Array>}
   */
  async glb({ scale = 1, metal, signal } = {}) {
    signal?.throwIfAborted();
    // A worker ended under it once is no reason to fail; twice running is.
    for (let attempt = 0; ; attempt++) {
      if (!page().holds(this.#kept)) {
        // The worker that held it was ended, or it was let go: strike it again.
        this.#kept = await strike(this.#svg, this.#options, signal);
      }
      const { worker } = this.#kept;
      try {
        return await worker.glb(this.#kept.id, scale, metal);
      } catch (e) {
        // A trap leaves the worker unfit for anything after.
        if (e?.trapped) page().discard(worker);
        if (!(e instanceof Gone || e?.gone) || attempt > 0) throw engineError(e);
        // Ended while it waited; try again with what replaces it.
      }
    }
  }
}

async function load(source, signal) {
  let bytes;
  if (typeof source === 'string') bytes = new TextEncoder().encode(source);
  else if (source instanceof Uint8Array) bytes = source;
  else if (source instanceof ArrayBuffer) bytes = new Uint8Array(source);
  else if (source instanceof URL) {
    if (source.protocol !== 'https:' && source.protocol !== 'http:') {
      throw new TypeError(`a web source is http(s), not ${source.protocol}`);
    }
    let loaded;
    try {
      loaded = await Luster.loader(source, signal);
    } catch (e) {
      if (e instanceof LusterError || signal?.aborted) throw e;
      throw new LusterError('unreadableSource', `could not fetch ${source}: ${e.message ?? e}`);
    }
    if (loaded instanceof Uint8Array) bytes = loaded;
    else if (loaded instanceof ArrayBuffer) bytes = new Uint8Array(loaded);
    else throw new TypeError('Luster.loader gives the bytes it fetched, a Uint8Array or an ArrayBuffer');
  } else {
    throw new TypeError('a source is SVG text, its bytes, or a URL');
  }
  if (bytes.length > MAXIMUM_BYTES) throw tooLarge(bytes.length);
  return bytes;
}

async function fetchBytes(url, signal) {
  const response = await fetch(url, { signal });
  if (!response.ok) throw new LusterError('unreadableSource', `could not fetch ${url}: HTTP ${response.status}`);
  const length = Number(response.headers.get('content-length'));
  if (length > MAXIMUM_BYTES) throw tooLarge(length);
  return new Uint8Array(await response.arrayBuffer());
}

function tooLarge(size) {
  return new LusterError('tooLarge', `the document is ${size} bytes; at most ${MAXIMUM_BYTES} are read`);
}
