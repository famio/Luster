// One worker running the engine, and the calls the page makes of it.

let compiled;

/** The engine's module, fetched and compiled once for every worker. */
export function compile() {
  return (compiled ??= (async () => {
    const url = new URL('../engine/luster_wasm_bg.wasm', import.meta.url);
    const response = fetch(url);
    try {
      return await WebAssembly.compileStreaming(response);
    } catch (e) {
      // Served as something other than application/wasm, it cannot stream.
      if (!(e instanceof TypeError)) throw e;
      const fetched = await response;
      if (!fetched.ok) throw new Error(`could not fetch ${url}: HTTP ${fetched.status}`);
      return WebAssembly.compile(await fetched.arrayBuffer());
    }
  })().catch((e) => {
    // A failure is not kept: the next mint tries again.
    compiled = undefined;
    throw e;
  }));
}

/** A call the worker could not answer because it was ended first. */
export class Gone extends Error {}

let bridge;

/** A worker loads only a script of the page's origin. Served from a CDN, the
 *  package starts one from a script of the page's own, a blob, that imports
 *  the worker. A bundler finds the worker by the first `new URL` and serves
 *  it with the page; webpack leaves a `file:` URL for this module. The blob's
 *  URL is spelt otherwise, so bundlers leave it be. */
function spawn() {
  const here = new URL(import.meta.url);
  if (!/^https?:$/.test(here.protocol) || here.origin === globalThis.location?.origin) {
    return new Worker(new URL('./worker.js', import.meta.url), { type: 'module' });
  }
  const worker = new URL(import.meta.url);
  worker.pathname = worker.pathname.replace(/[^/]*$/, 'worker.js');
  worker.search = worker.hash = '';
  bridge ??= URL.createObjectURL(new Blob([`import ${JSON.stringify(worker.href)};`], { type: 'text/javascript' }));
  return new Worker(bridge, { type: 'module' });
}

export class Engine {
  constructor() {
    this.worker = spawn();
    this.calls = new Map();
    this.serial = 0;
    this.worker.onmessage = ({ data }) => {
      const call = this.calls.get(data.call);
      if (!call) return;
      this.calls.delete(data.call);
      if (data.error) call.reject(data.error);
      else call.resolve(data.result);
    };
    // A worker that cannot load its script or its module answers nothing.
    this.worker.onerror = (event) => {
      event.preventDefault?.();
      this.end({ kind: 'engineFailure', message: event.message || 'the engine could not start', trapped: true });
    };
    // A worker without its engine is no use: it is ended, and the next one
    // tries again.
    this.ready = compile()
      .then((module) => this.call('init', { module }))
      .catch((e) => {
        throw e instanceof Gone || e?.kind ? e : { kind: 'engineFailure', message: `the engine could not start: ${e.message ?? e}`, trapped: true };
      });
    // Whoever calls next hears of it.
    this.ready.catch(() => {});
  }

  call(op, args = {}, transfer = []) {
    return new Promise((resolve, reject) => {
      // A worker ended, or one whose script never loaded, answers nothing.
      if (this.ended) return reject(this.ended);
      const call = ++this.serial;
      this.calls.set(call, { resolve, reject });
      this.worker.postMessage({ call, op, ...args }, transfer);
    });
  }

  /** Strikes a badge, which the worker keeps until `free`. */
  async mint(svg, options) {
    await this.ready;
    return this.call('mint', { svg, options });
  }

  async glb(id, scale, metal) {
    await this.ready;
    return this.call('glb', { id, scale, metal });
  }

  free(id) {
    this.ready.then(() => this.call('free', { id })).catch(() => {});
  }

  async run(op, args) {
    await this.ready;
    return this.call(op, args);
  }

  terminate() {
    this.worker.terminate();
    this.end(new Gone('the worker was ended'));
  }

  end(error) {
    this.ended ??= error;
    for (const { reject } of this.calls.values()) reject(error);
    this.calls.clear();
  }
}

/** A worker of its own for what is not a mint. */
let helper;

/** Asks the helper; one that fails is ended, and the next ask starts another. */
export async function ask(op, args) {
  const asked = (helper ??= new Engine());
  try {
    return await asked.run(op, args);
  } catch (e) {
    // Not one started since, after this one was ended.
    if (helper === asked) endHelper();
    throw e;
  }
}

export function endHelper() {
  helper?.terminate();
  helper = undefined;
}
