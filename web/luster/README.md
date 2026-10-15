# @famio/luster

The Luster engine compiled to WebAssembly: an SVG struck into a 3D enamel badge, in a
worker, off the thread that draws, and `<luster-view>` to show one. The badges are the
same bytes the engine makes on every other platform.

```sh
npm install @famio/luster three   # three only for the view and stills
```

Without a bundler, from a CDN that serves the package's files as published:

```html
<script type="importmap">
  { "imports": {
      "@famio/luster": "https://cdn.jsdelivr.net/npm/@famio/luster/src/index.js",
      "@famio/luster/view": "https://cdn.jsdelivr.net/npm/@famio/luster/src/view.js",
      "three": "https://cdn.jsdelivr.net/npm/three@0.186/build/three.module.js" } }
</script>
```

## Use

```js
import { Luster, LusterError } from '@famio/luster';

const badge = await Luster.mint(new URL('/badge.svg', location.href));
badge.submeshes;   // interleaved Float32Array vertices and Uint32Array indices
badge.materials;
badge.textures;    // RGBA8 with their mip chains
const glb = await badge.glb({ scale: 0.04 });
```

A source is SVG text, its bytes, or a `URL` object, fetched over http(s) with
`Luster.loader`. `Luster.mint` takes
`withoutHiddenFaces`, `metalLines` and an `AbortSignal`, and fails with a `LusterError`
whose `kind` says why. Two mints run at once across the page, the last eight badges are
kept, and asking for a badge already being minted waits for that mint. `Luster.close()`
ends the workers and lets go of their memory, which a WebAssembly module never gives
back while it lives.

The engine is fetched from `engine/luster_wasm_bg.wasm` next to the package, resolved
with `import.meta.url`; serve it as `application/wasm`, compressed, so it compiles as it
arrives. Vite and webpack bundle the worker and the engine as they are, served with the
page. Loaded from a CDN that serves the package's files as published, as jsDelivr's
`/npm/` and unpkg do, it starts its workers from a `blob:` script that imports them; a
CDN that rebundles modules moves the engine from beside them. A Content Security Policy
lets the engine compile with `script-src 'wasm-unsafe-eval'`, and the workers from a
CDN start with `worker-src blob:`.

Safari on iOS runs a page served over plain http from the local network without its
JIT, and a mint takes some fifteen times as long: try one on a phone over https.

## Show

```html
<script type="module">
  import '@famio/luster/view';
</script>

<luster-view src="badge.svg" metal="gold" style="width: 320px; height: 320px">
  <span class="spinner"></span>
</luster-view>
```

The view draws with [three.js](https://threejs.org), which the page provides (`three`
0.186, whose shader chunks it builds on; a page without a bundler maps it with an
import map). Drag to turn the badge; a flick keeps it spinning briefly, unless the
system asks for reduced motion or `momentum` is false. Pinch, on a trackpad or with
two fingers, to bring it up to five times closer, and slide two fingers to look about
it; a double click or tap puts it back, and `zoomable` set to false leaves it at its
size. Both are on unless their attribute says otherwise: `momentum="false"`,
`zoomable="false"`, as `draggable="false"` does. `metal` is `gold`, `silver`, `copper`
or a `#rrggbb` colour, and `lighting` is `showcase` or `off`; `metal-lines` and
`without-hidden-faces` are the mint's options, and the `source` property takes SVG
text or bytes in place of `src`.

The children are a placeholder, shown in the middle until the badge is on screen. The
element's `state` attribute is `minting`, `ready`, `failed` or `idle`, for styling it,
and each state it reports fires `statechange` with the state, the badge and the error.

Every view on the page draws in one WebGL context and copies its frame onto its own
canvas: a browser keeps only a few contexts a page, and a gallery of badges would lose
the first ones. The lighting is the studio every platform shares, its tone curve
RealityKit's.

## Stills

```js
import { LusterSnapshot } from '@famio/luster/snapshot';

const still = await LusterSnapshot.image(badge, { pixelSize: 512, metal: 'silver' });
canvas.getContext('bitmaprenderer').transferFromImageBitmap(still);

const file = await LusterSnapshot.blob(svg, { metalLines: true, type: 'image/webp' });
```

A still is the picture a view shows, at the resting pose unless `tilt` and `spin` say
otherwise. A list or a grid wants stills rather than a live view per row. `image` gives
an `ImageBitmap`, written into no file, for a canvas to draw; `blob` writes a PNG, or
the `type` asked for where the browser writes it (WebP in Chrome and Firefox; Safari
writes PNG), for saving or sending. Either takes a badge, or a source to strike one from
with the mint's options. A still is transparent round the badge unless it is given a
`background`, a `#rrggbb` colour or one in sRGB from 0 to 1. It draws with three.js, as
the view does.
