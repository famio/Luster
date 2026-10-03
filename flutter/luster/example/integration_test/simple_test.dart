import 'dart:io';
import 'dart:ui' as ui;

import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';
import 'package:luster/luster.dart';
import 'package:luster_example/main.dart' as app;

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();

  testWidgets('a badge is struck and shown', (tester) async {
    app.main();
    // The mint runs on the engine's threads, out of `pumpAndSettle`'s sight:
    // wait instead for the view to say the badge is ready.
    final ready = find.textContaining('ready ');
    for (var waited = 0; waited < 300 && ready.evaluate().isEmpty; waited++) {
      await tester.pump(const Duration(milliseconds: 100));
    }
    expect(find.byType(LusterView), findsOneWidget);
    expect(ready, findsOneWidget);

    // The engine answers, and the same document always gives the same key.
    final svg = (await rootBundle.load('assets/svg/sample-badge.svg'))
        .buffer
        .asUint8List();
    Future<LusterBadge> mint({bool metalLines = false}) =>
        Luster.mint(LusterSource.bytes(svg), options: LusterOptions(metalLines: metalLines));
    final badge = await mint();
    // The key names the document, the options and the engine's version, so
    // what is checked is that it is stable, not what it is.
    expect(badge.designKey, hasLength(16));
    expect((await mint()).designKey, badge.designKey);
    expect((await mint(metalLines: true)).designKey, isNot(badge.designKey));

    // Asked for together, both callers get the badge.
    final other = Uint8List.fromList([...svg, ...'<!-- again -->'.codeUnits]);
    final both = await Future.wait(
        [Luster.mint(LusterSource.bytes(other)), Luster.mint(LusterSource.bytes(other))]);
    expect(both[1], both[0]);

    // The GLB button's bytes: a glTF binary, the same every time for the
    // same badge and metal, and another metal plates it differently.
    final glb = await badge.glb();
    expect(String.fromCharCodes(glb.sublist(0, 4)), 'glTF');
    expect(await badge.glb(), glb);
    expect(await badge.glb(metal: LusterColor.silver),
        isNot(glb));

    // A still: clear round the badge unless given a background, covered in
    // the middle, as the view shows it.
    Future<ByteData> pixels(ui.Image image) async =>
        (await image.toByteData(format: ui.ImageByteFormat.rawRgba))!;
    int alpha(ByteData data, int x, int y) => data.getUint8((y * 256 + x) * 4 + 3);
    final still = await LusterSnapshot.image(badge, pixelSize: 256);
    expect(still.width, 256);
    final clear = await pixels(still);
    expect(alpha(clear, 0, 0), 0);
    expect(alpha(clear, 128, 128), 255);
    final card = await pixels(await LusterSnapshot.image(badge,
        pixelSize: 256, background: const LusterColor(1, 1, 1)));
    expect(alpha(card, 0, 0), 255);
    expect(card.getUint8(0), 255);
    // `--dart-define=LUSTER_STILL=<path>` keeps one, to look at.
    const shot = String.fromEnvironment('LUSTER_STILL');
    if (shot.isNotEmpty) {
      final png = await (await LusterSnapshot.fromSource(LusterSource.bytes(svg), pixelSize: 768))
          .toByteData(format: ui.ImageByteFormat.png);
      File(shot).writeAsBytesSync(png!.buffer.asUint8List());
    }

    // A web source goes through the loader the app sets.
    final previous = Luster.loader;
    Luster.loader = (url) async => url.path == '/badge.svg'
        ? svg
        : throw const HttpException('not found');
    try {
      final fetched = await Luster.mint(LusterSource.url(Uri.parse('https://luster.test/badge.svg')));
      expect(fetched, badge);
      await expectLater(Luster.mint(LusterSource.url(Uri.parse('https://luster.test/missing.svg'))),
          throwsA(isA<LusterException>()
              .having((e) => e.kind, 'kind', LusterErrorKind.unreadableSource)));
    } finally {
      Luster.loader = previous;
    }

  });
}
