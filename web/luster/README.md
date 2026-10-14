# luster (web)

The Luster engine compiled to WebAssembly: an SVG struck into a 3D enamel badge, in a
worker, off the thread that draws. The badges are the same bytes the engine makes on
every other platform.

Not published yet. Build the engine into `engine/` first:

```sh
Scripts/build-wasm.sh
```

## Use

```js
import { Luster, LusterError } from 'luster';

const badge = await Luster.mint(new URL('/badge.svg', location.href));
badge.submeshes;   // interleaved Float32Array vertices and Uint32Array indices
badge.materials;
badge.textures;    // RGBA8 with their mip chains
const glb = await badge.glb({ scale: 0.04 });
```

A source is SVG text, its bytes, or an http(s) URL. `Luster.mint` takes
`withoutHiddenFaces`, `metalLines` and an `AbortSignal`, and fails with a `LusterError`
whose `kind` says why. Two mints run at once across the page, the last eight badges are
kept, and asking for a badge already being minted waits for that mint. `Luster.close()`
ends the workers and lets go of their memory, which a WebAssembly module never gives
back while it lives.

The engine is fetched from `engine/luster_wasm_bg.wasm` next to the package, resolved
with `import.meta.url`; serve it as `application/wasm`, compressed, so it compiles as it
arrives.

## Checks

```sh
npm test    # the mints' scheduling, on workers that are only promises
```
