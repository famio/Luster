// The page's mints: two at once, the last eight badges kept, and one mint for
// everyone asking for the same badge, as the engine's other bindings have
// them (luster-core/src/mints.rs). A mint runs in a worker, which goes on
// holding the engine's badge for the GLB; ending the worker is how a mint is
// given up, once everyone waiting for it has.

/** @typedef {{ withoutHiddenFaces: boolean, metalLines: boolean }} Options */

export class Mints {
  /**
   * @param {object} how
   * @param {() => import('./engine.js').Engine} how.spawn a new worker
   * @param {number} [how.running] mints at once
   * @param {number} [how.kept] badges kept, the least recently asked for
   *   going first
   */
  constructor({ spawn, running = 2, kept = 8 }) {
    this.spawn = spawn;
    this.running = running;
    this.kept = kept;
    /** Workers with nothing to do. */
    this.idle = [];
    /** Every worker alive, idle or not. */
    this.workers = new Set();
    /** @type {Map<number, Flight[]>} mints asked for, by key hash */
    this.flights = new Map();
    /** @type {Flight[]} waiting for a worker, oldest first */
    this.queue = [];
    /** @type {Kept[]} least recently asked for first */
    this.recent = [];
  }

  /**
   * The badge for `svg` struck with `options`, from those kept, the mint
   * already under way, or a new one.
   * @param {Uint8Array} svg not changed while the mint runs
   * @param {Options} options
   * @param {AbortSignal} [signal]
   * @returns {Promise<Kept>}
   */
  request(svg, options, signal) {
    if (signal?.aborted) return Promise.reject(signal.reason);
    const hash = keyHash(svg, options);
    const kept = this.recent.findIndex((k) => k.hash === hash && sameKey(k, svg, options));
    if (kept >= 0) {
      const [hit] = this.recent.splice(kept, 1);
      this.recent.push(hit);
      return Promise.resolve(hit);
    }
    let flight = (this.flights.get(hash) ?? []).find((f) => sameKey(f, svg, options));
    if (!flight) {
      flight = new Flight(hash, svg.slice(), options);
      this.flights.set(hash, [...(this.flights.get(hash) ?? []), flight]);
      this.queue.push(flight);
      this.pump();
    }
    return flight.wait(signal, () => this.giveUp(flight));
  }

  /** Starts what there are workers for. */
  pump() {
    while (this.queue.length && this.workers.size - this.idle.length < this.running) {
      const flight = this.queue.shift();
      const worker = this.idle.pop() ?? this.hire();
      flight.worker = worker;
      worker.mint(flight.svg, flight.options).then(
        ({ id, badge }) => this.land(flight, worker, id, badge),
        (error) => this.fail(flight, worker, error),
      );
    }
  }

  hire() {
    const worker = this.spawn();
    this.workers.add(worker);
    return worker;
  }

  /** Ends a worker, and with it everything it holds. */
  fire(worker) {
    worker.terminate();
    this.workers.delete(worker);
    this.idle = this.idle.filter((w) => w !== worker);
    this.recent = this.recent.filter((k) => k.worker !== worker);
  }

  land(flight, worker, id, badge) {
    // A worker ended while it minted has nothing to hand over.
    if (!this.workers.has(worker)) return;
    this.forget(flight);
    this.idle.push(worker);
    const kept = { hash: flight.hash, svg: flight.svg, options: flight.options, badge, worker, id };
    this.recent.push(kept);
    while (this.recent.length > this.kept) {
      const old = this.recent.shift();
      old.worker.free(old.id);
    }
    flight.settle(kept, null);
    this.pump();
  }

  fail(flight, worker, error) {
    if (!this.workers.has(worker)) return;
    this.forget(flight);
    // A trap leaves the module unfit to call again.
    if (error.trapped) this.fire(worker);
    else this.idle.push(worker);
    flight.settle(null, error);
    this.pump();
  }

  /** Everyone has given the flight up. */
  giveUp(flight) {
    this.forget(flight);
    this.queue = this.queue.filter((f) => f !== flight);
    if (flight.worker) this.fire(flight.worker);
    this.pump();
  }

  forget(flight) {
    const same = (this.flights.get(flight.hash) ?? []).filter((f) => f !== flight);
    if (same.length) this.flights.set(flight.hash, same);
    else this.flights.delete(flight.hash);
  }

  /** Whether `kept` still has the worker that holds its badge. */
  holds(kept) {
    return this.recent.includes(kept);
  }

  /** Ends every worker: what they hold and what they were doing. */
  close() {
    for (const worker of [...this.workers]) this.fire(worker);
    for (const flights of this.flights.values()) {
      for (const flight of flights) flight.settle(null, new DOMException('closed', 'AbortError'));
    }
    this.flights.clear();
    this.queue = [];
  }
}

/**
 * @typedef {object} Kept
 * @property {number} hash
 * @property {Uint8Array} svg
 * @property {Options} options
 * @property {object} badge what the worker sent
 * @property {object} worker
 * @property {number} id the badge's in the worker
 */

class Flight {
  constructor(hash, svg, options) {
    this.hash = hash;
    this.svg = svg;
    this.options = options;
    this.worker = null;
    this.waiting = 0;
    /** @type {Set<{resolve: Function, reject: Function}>} */
    this.waiters = new Set();
  }

  wait(signal, abandoned) {
    return new Promise((resolve, reject) => {
      const waiter = { resolve, reject };
      this.waiters.add(waiter);
      this.waiting++;
      signal?.addEventListener(
        'abort',
        () => {
          if (!this.waiters.delete(waiter)) return;
          reject(signal.reason);
          if (--this.waiting === 0) abandoned();
        },
        { once: true },
      );
    });
  }

  settle(kept, error) {
    for (const { resolve, reject } of this.waiters) {
      if (error) reject(error);
      else resolve(kept);
    }
    this.waiters.clear();
  }
}

/** FNV-1a over the document and the options: which keys to compare. */
function keyHash(svg, options) {
  let h = 0x811c9dc5;
  for (let i = 0; i < svg.length; i++) h = Math.imul(h ^ svg[i], 0x01000193);
  h = Math.imul(h ^ (options.withoutHiddenFaces ? 1 : 0), 0x01000193);
  return Math.imul(h ^ (options.metalLines ? 1 : 0), 0x01000193) >>> 0;
}

function sameKey(entry, svg, options) {
  if (
    entry.options.withoutHiddenFaces !== options.withoutHiddenFaces ||
    entry.options.metalLines !== options.metalLines ||
    entry.svg.length !== svg.length
  ) {
    return false;
  }
  for (let i = 0; i < svg.length; i++) if (entry.svg[i] !== svg[i]) return false;
  return true;
}
