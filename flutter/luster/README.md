# luster

Strikes an SVG into a 3D gold enamel badge, and shows it lit and turning.

Hand it a document and Luster builds the whole object from its paths — the
silhouette, the enamel cells, the raised gold line work, the rolled edge, the
sandblasted reverse — then draws it with flutter_scene or writes it out as GLB.
It ships no 3D model assets and no artwork: everything comes from the document.

The engine is Rust, shared with Luster's Swift package and Kotlin library, so the
same document gives the same badge, to the byte, on every platform.

## Setup

The widget draws with Flutter GPU, which the app has to turn on:

```xml
<!-- ios/Runner/Info.plist -->
<key>FLTEnableFlutterGPU</key>
<true/>
```

```xml
<!-- android/app/src/main/AndroidManifest.xml, inside <application> -->
<meta-data android:name="io.flutter.embedding.android.EnableFlutterGPU" android:value="true" />
<meta-data android:name="io.flutter.embedding.android.ImpellerBackend" android:value="vulkan" />
```

On Android Flutter GPU wants the Vulkan backend. The standard emulator images
run Impeller on OpenGL ES, where the scene never comes up, so try it on a device.

The engine comes prebuilt for iOS, macOS and Android, and signed: the package
checks each file against its key before using it, so the app needs no Rust.
Where `rustup` is installed the engine is built from source instead, unless a
`cargokit_options.yaml` at the app's root asks for the prebuilt one:

```yaml
use_precompiled_binaries: true
```

## Use

```dart
import 'package:luster/luster.dart';

LusterView(
  source: LusterSource.url(Uri.parse('https://example.com/badge.svg')),
  options: const LusterOptions(metalLines: true),
  appearance: LusterAppearance(metal: LusterColor.silver),
  onStateChange: (state) => switch (state) {
    LusterReady(:final badge) => print(badge.designKey),
    LusterFailed(:final error) => print(error), // a LusterException, with its kind
    _ => null,
  },
)
```

Drag to turn the badge; it keeps going with momentum, unless the system asks for
reduced motion or `momentumEnabled` is false. The widget paints nothing behind
the badge: put it in a `ColoredBox`, or in a `Stack` over anything, for a
background.

Minting runs on the engine's own threads, so the UI isolate keeps its frames, and
the previous badge stays on screen while a new one is struck. A different `source`
or `options` mints again; a different `appearance` only re-lights and re-plates.

| What | Type |
| --- | --- |
| The document | `LusterSource.url(uri)`, `.bytes(bytes)`, `.svg(text)` |
| What to strike | `LusterOptions(metalLines:, withoutHiddenFaces:)` |
| How to light it | `LusterAppearance(metal:, lighting:)`, with `LusterColor.gold / silver / copper` or any `LusterColor`, and `LusterLighting.showcase / off` |
| What it is doing | `LusterIdle`, `LusterMinting`, `LusterReady(badge)`, `LusterFailed(error)` |

`Luster.init()` loads the engine and flutter_scene's shaders. The widget calls it
itself; calling it at launch gets it done before the first badge is wanted. The
named colours are the engine's, so they are there once it has loaded.

### Without a view

```dart
final badge = await Luster.mint(LusterSource.bytes(bytes),
    options: const LusterOptions(metalLines: true));
final glb = await badge.glb(metal: LusterColor.gold);          // glTF binary
final still = await LusterSnapshot.image(badge, pixelSize: 512); // ui.Image
```

At most two mints run at once across the process, and recent badges are kept,
keyed by the document's bytes and the options, so asking again is immediate;
asking for one already being struck waits for that mint rather than starting
another. A badge's `designKey` names the document and the options it was struck
with: equal keys mean the same badge.

A mint that fails throws a `LusterException`, whose `kind` says why:
`invalidSvg`, `inputTooComplex` or `nothingToMint` for the document, `tooLarge`
or `unreadableSource` for where it came from, `engineFailure` for a bug.

A list or a grid wants stills, not a view per row. A still is drawn as the view
draws, and has no background unless it is given one: its alpha is the badge's
coverage. `background:` takes a colour to lay it over instead.

`badge.glb()` writes the same bytes the other platforms write for the same
badge and metal, so two exports can be compared directly.

### From the web

A web source is fetched with `Luster.loader`, by default `dart:io`'s `HttpClient`,
which keeps nothing; documents over 20 MB are refused, and on Android the app needs
the `INTERNET` permission. To fetch through
flutter_cache_manager, as cached_network_image does:

```dart
import 'package:flutter_cache_manager/flutter_cache_manager.dart';

Luster.loader = (url) async =>
    (await DefaultCacheManager().getSingleFile(url.toString())).readAsBytes();
```

A source that cannot be fetched fails with a `LusterException` whose `kind` is
`unreadableSource` (`tooLarge` past the limit). Whatever the
loader, the engine keeps recent badges by the document's bytes, so a document
fetched again is not struck again.

## How it is drawn

The engine hands over the meshes and one sheet of colours, and the widget builds
the scene in Dart with flutter_scene, not a platform view.

flutter_scene's tone curves keep a colour's saturation as it brightens, where
RealityKit's let it fade towards white. The widget draws with no curve of its
own and reads the result through RealityKit's, measured into a table, so a badge
shows the colours it shows on the Apple side.

The badge is drawn every frame only while it turns; at rest it is drawn once,
when something changes. Under the showcase it is drawn with its own shader
(`shaders/luster_badge.frag`, compiled by `hook/build.dart`): flutter_scene's
standard image-based lighting and nothing else, several times cheaper a pixel
than the standard material. On a GPU too slow for full resolution the widget
starts each turn coarser and comes down further while frames come too slowly.

The package pins the Flutter version it is built against in `.fvmrc`.

## License

MIT. See [How it works](https://github.com/famio/Luster/blob/main/docs/how-it-works.md) for how
the badge is made.
