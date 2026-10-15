// Before the package is packed: the licences beside it, as npm takes no file
// from outside it, and a check that the engine it carries is this version's,
// built for release.
import { copyFileSync, existsSync, readFileSync, statSync } from 'node:fs';

const here = (path) => new URL(`../${path}`, import.meta.url);
const root = new URL('../../../', import.meta.url);

for (const name of ['LICENSE', 'THIRD_PARTY_NOTICES.md']) copyFileSync(new URL(name, root), here(name));

const wasm = here('engine/luster_wasm_bg.wasm');
if (!existsSync(wasm)) throw new Error('no engine: run Scripts/build-wasm.sh first');
// A debug build is several times a release's size.
if (statSync(wasm).size > 4 << 20) throw new Error('the engine is a debug build: run Scripts/build-wasm.sh');
const { initSync, version } = await import(here('engine/luster_wasm.js'));
initSync({ module: readFileSync(wasm) });
const wanted = JSON.parse(readFileSync(here('package.json'))).version;
if (version() !== wanted) {
  throw new Error(`the engine is ${version()}, the package ${wanted}: run Scripts/build-wasm.sh`);
}
