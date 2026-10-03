# luster_example

The Flutter demo, the same as the iOS and Android ones: open an SVG, pick the
lines, the metal and the light, and save a GLB.

```sh
fvm flutter run   # --dart-define=LUSTER_URL=<url> opens on a document on the web
```

On Android, run it on a device: Flutter GPU wants Vulkan, which the standard
emulator images do not give it.

`integration_test/simple_test.dart` strikes a badge, takes stills of it and
writes its GLB; `--dart-define=LUSTER_STILL=<path>` keeps a still to look at.
