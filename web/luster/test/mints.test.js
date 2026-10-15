// The page's mints, on workers that are only promises.
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { Mints } from '../src/mints.js';

/** A worker whose mints finish when the test says. */
class FakeWorker {
  static all = [];
  constructor() {
    this.mints = [];
    this.freed = [];
    this.ended = false;
    FakeWorker.all.push(this);
  }
  mint(svg, options) {
    return new Promise((resolve, reject) => this.mints.push({ svg, options, resolve, reject }));
  }
  free(id) {
    this.freed.push(id);
  }
  terminate() {
    this.ended = true;
    for (const m of this.mints) m.reject(new Error('gone'));
  }
}

const doc = (n) => new TextEncoder().encode(`<svg id="${n}"/>`);
const plain = { withoutHiddenFaces: false, metalLines: false };
const tick = () => new Promise((r) => setTimeout(r, 0));
const fresh = (how = {}) => {
  FakeWorker.all = [];
  return new Mints({ spawn: () => new FakeWorker(), ...how });
};
const finish = (worker, i, id = i + 1) => worker.mints[i].resolve({ id, badge: { designKey: `k${id}` } });

test('one mint for everyone asking for the same badge', async () => {
  const mints = fresh();
  const a = mints.request(doc(1), plain);
  const b = mints.request(doc(1), { ...plain });
  assert.equal(FakeWorker.all.length, 1);
  assert.equal(FakeWorker.all[0].mints.length, 1);
  finish(FakeWorker.all[0], 0);
  assert.equal(await a, await b);
});

test('the options are part of the key', async () => {
  const mints = fresh();
  mints.request(doc(1), plain);
  mints.request(doc(1), { ...plain, metalLines: true });
  assert.equal(FakeWorker.all.length, 2);
});

test('two at once, the rest in turn', async () => {
  const mints = fresh();
  const asked = [1, 2, 3].map((n) => mints.request(doc(n), plain));
  assert.equal(FakeWorker.all.length, 2);
  assert.equal(FakeWorker.all.reduce((n, w) => n + w.mints.length, 0), 2);
  finish(FakeWorker.all[0], 0);
  await asked[0];
  // The third goes to the worker the first left.
  assert.equal(FakeWorker.all.length, 2);
  assert.equal(FakeWorker.all[0].mints.length, 2);
});

test('a kept badge is given again without a mint, and the oldest is let go', async () => {
  const mints = fresh({ running: 1, kept: 2 });
  const worker = () => FakeWorker.all[0];
  for (const n of [1, 2, 3]) {
    const asked = mints.request(doc(n), plain);
    finish(worker(), n - 1, n);
    await asked;
  }
  assert.deepEqual(worker().freed, [1]);
  const again = await mints.request(doc(2), plain);
  assert.equal(again.id, 2);
  assert.equal(worker().mints.length, 3);
  assert.ok(mints.holds(again));
});

test('a mint stops only when everyone has given it up', async () => {
  const mints = fresh();
  const one = new AbortController();
  const two = new AbortController();
  const a = mints.request(doc(1), plain, one.signal);
  const b = mints.request(doc(1), plain, two.signal);
  one.abort();
  await assert.rejects(a, { name: 'AbortError' });
  assert.equal(FakeWorker.all[0].ended, false);
  two.abort();
  await assert.rejects(b, { name: 'AbortError' });
  assert.equal(FakeWorker.all[0].ended, true);
  // The next mint has a worker of its own.
  mints.request(doc(1), plain);
  assert.equal(FakeWorker.all.length, 2);
  assert.equal(FakeWorker.all[1].mints.length, 1);
});

test('a mint given up while it waits never starts', async () => {
  const mints = fresh({ running: 1 });
  mints.request(doc(1), plain);
  const stop = new AbortController();
  const waiting = mints.request(doc(2), plain, stop.signal);
  stop.abort();
  await assert.rejects(waiting, { name: 'AbortError' });
  finish(FakeWorker.all[0], 0);
  await tick();
  assert.equal(FakeWorker.all[0].mints.length, 1);
});

test('a trap ends the worker and what it held; a document error does not', async () => {
  const mints = fresh({ running: 1 });
  const kept = mints.request(doc(1), plain);
  finish(FakeWorker.all[0], 0);
  const held = await kept;
  const bad = mints.request(doc(2), plain);
  FakeWorker.all[0].mints[1].reject({ kind: 'invalidSvg', message: 'no' });
  await assert.rejects(bad, { kind: 'invalidSvg' });
  assert.equal(FakeWorker.all[0].ended, false);
  const trap = mints.request(doc(3), plain);
  FakeWorker.all[0].mints[2].reject({ kind: 'engineFailure', message: 'unreachable', trapped: true });
  await assert.rejects(trap, { kind: 'engineFailure' });
  assert.equal(FakeWorker.all[0].ended, true);
  assert.equal(mints.holds(held), false);
  mints.request(doc(1), plain);
  assert.equal(FakeWorker.all.length, 2);
});

test('closing rejects what is under way', async () => {
  const mints = fresh();
  const asked = mints.request(doc(1), plain);
  mints.close();
  await assert.rejects(asked, { name: 'AbortError' });
  assert.equal(FakeWorker.all[0].ended, true);
});

test('a worker found unfit is ended, and its badges and its mint go elsewhere', async () => {
  const mints = fresh();
  const kept = mints.request(doc(1), plain);
  finish(FakeWorker.all[0], 0);
  const held = await kept;
  const asked = mints.request(doc(2), plain);
  const [unfit] = FakeWorker.all;
  assert.equal(unfit.mints.length, 2);
  mints.discard(unfit);
  assert.ok(unfit.ended);
  assert.ok(!mints.holds(held));
  // The second mint starts again on a worker of its own.
  await tick();
  const fresh2 = FakeWorker.all.at(-1);
  assert.notEqual(fresh2, unfit);
  finish(fresh2, 0, 7);
  assert.equal((await asked).id, 7);
});

test('a settled mint lets go of the signal it was given', async () => {
  const mints = fresh();
  const controller = new AbortController();
  let listening = 0;
  const signal = controller.signal;
  const add = signal.addEventListener.bind(signal);
  const remove = signal.removeEventListener.bind(signal);
  signal.addEventListener = (...a) => (listening++, add(...a));
  signal.removeEventListener = (...a) => (listening--, remove(...a));
  const asked = mints.request(doc(1), plain, signal);
  assert.equal(listening, 1);
  finish(FakeWorker.all[0], 0);
  await asked;
  assert.equal(listening, 0);
});
