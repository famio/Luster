The app has to turn on Flutter GPU first; the package's README shows how.

## A badge on screen

An SVG from the app's assets (listed under `flutter: assets:` in its pubspec),
struck into a badge that turns under the finger, with a spinner until it is up:

```dart
import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:luster/luster.dart';

void main() {
  // Loads the engine while the first frames show; the view waits for it.
  unawaited(Luster.init());
  runApp(const MaterialApp(home: BadgePage()));
}

class BadgePage extends StatefulWidget {
  const BadgePage({super.key});

  @override
  State<BadgePage> createState() => _BadgePageState();
}

class _BadgePageState extends State<BadgePage> {
  LusterSource? source;

  @override
  void initState() {
    super.initState();
    rootBundle.loadString('assets/badge.svg').then((svg) {
      if (mounted) setState(() => source = LusterSource.svg(svg));
    });
  }

  @override
  Widget build(BuildContext context) => Scaffold(
    body: LusterView(
      source: source,
      placeholder: (context, state) => state is LusterFailed
          ? const Icon(Icons.error_outline)
          : const CircularProgressIndicator(),
    ),
  );
}
```

## Without a view

The same badge, struck without a view, written out as GLB and taken as a still:
for a list of thumbnails, or to send the badge somewhere else.

```dart
final badge = await Luster.mint(LusterSource.svg(svg));
final glb = await badge.glb();                                   // glTF binary
final still = await LusterSnapshot.image(badge, pixelSize: 256); // ui.Image
```

`lib/main.dart` is the full demo: open an SVG, pick the lines, the metal and the
light, and save a GLB.
