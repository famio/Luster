// Runs a WASI command-line program under Node, for Scripts/parity.sh: it sees
// the repository at the same path, and nothing else.
//
//   node Scripts/wasi.mjs <program.wasm> <arguments>…
import { readFile } from 'node:fs/promises';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { WASI } from 'node:wasi';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const [program, ...args] = process.argv.slice(2);
const wasi = new WASI({
  version: 'preview1',
  args: [program, ...args],
  preopens: { [root]: root },
  returnOnExit: true,
});
const module = await WebAssembly.compile(await readFile(program));
process.exitCode = wasi.start(await WebAssembly.instantiate(module, wasi.getImportObject()));
