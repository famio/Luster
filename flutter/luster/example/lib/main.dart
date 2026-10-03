import 'dart:async';
import 'dart:io';

import 'package:file_picker/file_picker.dart';
import 'package:flutter/cupertino.dart' show CupertinoIcons;
import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter/scheduler.dart';
import 'package:flutter/services.dart';
import 'package:luster/luster.dart';

/// `--dart-define=LUSTER_CYCLE=true` mints the sample repeatedly and prints the
/// longest gap between frames on the UI isolate while each mint ran, then
/// exits. The UI is kept drawing while a mint runs, so a long gap is a hitch.
const cycle = bool.fromEnvironment('LUSTER_CYCLE');

/// `LUSTER_LIGHTING=off` in the environment opens on the lamps' light, for
/// photographing the same badge the same way on every platform.
final startingLighting = Platform.environment['LUSTER_LIGHTING'] ?? '';

/// The document the demo opens on; any other is opened from a file.
const sample = (asset: 'assets/svg/namiura.svg', title: 'Namiura');

/// `--dart-define=LUSTER_URL=<url>` opens on a document on the web instead.
const startingUrl = String.fromEnvironment('LUSTER_URL');

void main() {
  // Start the engine without waiting: the badge view waits for it, so the rest
  // of the UI shows at once.
  unawaited(Luster.init());
  runApp(const DemoApp());
}

/// Follows the system's light or dark appearance, in greys, as the iOS demo
/// does.
class DemoApp extends StatelessWidget {
  const DemoApp({super.key});

  @override
  Widget build(BuildContext context) => MaterialApp(
        debugShowCheckedModeBanner: false,
        title: 'Luster',
        theme: _theme(Brightness.light),
        darkTheme: _theme(Brightness.dark),
        home: const DemoPage(),
      );

  static ThemeData _theme(Brightness brightness) => ThemeData(
        colorScheme: ColorScheme.fromSeed(
          seedColor: Colors.grey,
          brightness: brightness,
          dynamicSchemeVariant: DynamicSchemeVariant.monochrome,
        ),
        scaffoldBackgroundColor:
            brightness == Brightness.dark ? Colors.black : Colors.white,
      );
}

/// The choices the menus offer, as the iOS demo names them.
const lines = [('Plated', false), ('Metal', true)];
/// Each metal is read from the engine when it is used, as the engine may
/// still be loading while the menus show.
final metals = <(String, LusterColor? Function())>[
  ('Gold', () => null), // the engine's own
  ('Silver', () => LusterColor.silver),
  ('Copper', () => LusterColor.copper),
];
const lightings = [
  ('Showcase', LusterLighting.showcase),
  ('Off', LusterLighting.off),
];

class DemoPage extends StatefulWidget {
  const DemoPage({super.key});

  @override
  State<DemoPage> createState() => _DemoPageState();
}

class _DemoPageState extends State<DemoPage> {
  /// The document on screen: bytes from the bundle or a file, or a web URL.
  LusterSource? source;
  set svg(Uint8List bytes) => source = LusterSource.bytes(bytes);
  String svgTitle = sample.title;
  bool metalLines = false;
  String metal = metals.first.$1;
  LusterLighting lighting = switch (startingLighting) {
    'off' => LusterLighting.off,
    _ => LusterLighting.showcase,
  };
  String status = 'loading';
  /// The badge on screen, once it is ready: what the GLB button writes out.
  LusterBadge? badge;
  bool exporting = false;
  Duration longestFrame = Duration.zero;
  Duration? lastFrame;
  bool minting = false;
  /// Times a mint from the moment it is asked for.
  final Stopwatch mintWatch = Stopwatch();

  @override
  void initState() {
    super.initState();
    SchedulerBinding.instance.addPersistentFrameCallback((stamp) {
      final last = lastFrame;
      if (last != null && stamp - last > longestFrame) longestFrame = stamp - last;
      lastFrame = stamp;
      // A badge at rest is not redrawn, so nothing else asks for frames while
      // a mint runs; asking here keeps them coming.
      if (minting) SchedulerBinding.instance.scheduleFrame();
    });
    if (startingUrl.isNotEmpty) {
      final url = Uri.parse(startingUrl);
      svgTitle = url.pathSegments.last.replaceFirst(RegExp(r'\.svg$', caseSensitive: false), '');
      source = LusterSource.url(url);
      return;
    }
    rootBundle.load(sample.asset).then((data) {
      setState(() => svg = data.buffer.asUint8List());
      if (cycle) _cycle(data.buffer.asUint8List());
    });
  }

  /// Mints fresh copies of the sample (so nothing is cached) and reports the
  /// longest frame while each was in flight.
  Future<void> _cycle(Uint8List base) async {
    await Future<void>.delayed(const Duration(seconds: 3));
    debugPrint('round\tmint+scene ms\tlongest frame gap ms');
    for (var round = 0; round < 6; round++) {
      final bytes = Uint8List.fromList([...base, ...'<!-- $round -->'.codeUnits]);
      longestFrame = Duration.zero;
      final watch = Stopwatch()..start();
      setState(() => svg = bytes);
      while (!status.startsWith('ready')) {
        await Future<void>.delayed(const Duration(milliseconds: 5));
        if (watch.elapsed > const Duration(seconds: 10)) break;
      }
      await Future<void>.delayed(const Duration(milliseconds: 200));
      debugPrint('$round\t${watch.elapsedMilliseconds}\t'
          '${(longestFrame.inMicroseconds / 1000).toStringAsFixed(1)}');
      status = '';
    }
    debugPrint('done');
    exit(0);
  }

  /// Puts an SVG the user picks on the badge, under its name.
  Future<void> _open() async {
    try {
      final file =
          await FilePicker.pickFile(type: FileType.custom, allowedExtensions: ['svg']);
      if (file == null) return;
      final bytes = await file.readAsBytes();
      setState(() {
        svg = bytes;
        svgTitle = file.name.replaceFirst(RegExp(r'\.svg$', caseSensitive: false), '');
      });
    } catch (error) {
      setState(() => status = 'failed: $error');
    }
  }

  /// Writes the badge on screen as a GLB, in the metal it is shown in, where
  /// the user says.
  Future<void> _export() async {
    final badge = this.badge;
    if (badge == null) return;
    setState(() => exporting = true);
    try {
      final bytes = await badge.glb(metal: metals.firstWhere((m) => m.$1 == metal).$2());
      final saved = await FilePicker.saveFile(
        fileName: '$svgTitle.glb',
        bytes: bytes,
        mimeType: 'model/gltf-binary',
        type: FileType.custom,
        allowedExtensions: ['glb'],
      );
      if (saved != null) {
        setState(() => status = 'exported $svgTitle.glb · ${bytes.length ~/ 1024} KB');
      }
    } catch (error) {
      setState(() => status = 'failed: $error');
    } finally {
      if (mounted) setState(() => exporting = false);
    }
  }

  void _onState(LusterState state) => setState(() {
        badge = null;
        minting = state is LusterMinting;
        switch (state) {
          case LusterIdle():
            status = 'idle';
          case LusterMinting():
            longestFrame = Duration.zero;
            lastFrame = null;
            mintWatch
              ..reset()
              ..start();
            status = 'minting…';
          case LusterReady(:final badge):
            this.badge = badge;
            status = 'ready ${badge.designKey.substring(0, 6)} · '
                '${mintWatch.elapsedMilliseconds} ms · longest frame gap '
                '${longestFrame.inMilliseconds} ms';
          case LusterFailed(:final error):
            status = 'failed: $error';
        }
      });

  @override
  Widget build(BuildContext context) {
    final scheme = ColorScheme.of(context);
    return Scaffold(
      body: Column(
        children: [
          Expanded(
            child: LusterView(
              source: source,
              options: LusterOptions(metalLines: metalLines),
              appearance: LusterAppearance(
                metal: metals.firstWhere((m) => m.$1 == metal).$2(),
                lighting: lighting,
              ),
              onStateChange: _onState,
            ),
          ),
          SafeArea(
            top: false,
            minimum: const EdgeInsets.only(bottom: 16),
            child: Padding(
              padding: const EdgeInsets.fromLTRB(16, 16, 16, 0),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  Row(
                    spacing: 4,
                    children: [
                      Expanded(
                        child: BarButton(
                          icon: CupertinoIcons.folder,
                          label: svgTitle,
                          onPressed: _open,
                        ),
                      ),
                      Expanded(
                        child: ChoiceMenu(
                          title: 'Lines',
                          icon: CupertinoIcons.scribble,
                          choices: lines,
                          selected: metalLines,
                          onSelected: (v) => setState(() => metalLines = v),
                        ),
                      ),
                      Expanded(
                        child: ChoiceMenu(
                          title: 'Metal',
                          icon: CupertinoIcons.paintbrush,
                          choices: [for (final (name, _) in metals) (name, name)],
                          selected: metal,
                          // Only gold is shown until the engine is ready.
                          onSelected: (v) async {
                            await Luster.init();
                            if (mounted) setState(() => metal = v);
                          },
                        ),
                      ),
                      Expanded(
                        child: ChoiceMenu(
                          title: 'Light',
                          icon: CupertinoIcons.lightbulb,
                          choices: lightings,
                          selected: lighting,
                          onSelected: (v) => setState(() => lighting = v),
                        ),
                      ),
                      Expanded(
                        child: BarButton(
                          icon: CupertinoIcons.square_arrow_up,
                          label: 'GLB',
                          // Until a badge is ready there is nothing to write.
                          onPressed: badge == null || exporting ? null : _export,
                        ),
                      ),
                    ],
                  ),
                  const SizedBox(height: 8),
                  Text(
                    status,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: TextStyle(
                      fontFamily: 'Menlo',
                      fontFamilyFallback: const ['monospace'],
                      fontSize: 11,
                      color: scheme.onSurfaceVariant,
                    ),
                  ),
                ],
              ),
            ),
          ),
        ],
      ),
    );
  }
}

/// A grey capsule with an icon over a caption, like the iOS demo's buttons.
class BarButton extends StatelessWidget {
  const BarButton({
    super.key,
    required this.icon,
    required this.label,
    required this.onPressed,
  });

  final IconData icon;
  final String label;
  final VoidCallback? onPressed;

  @override
  Widget build(BuildContext context) {
    final scheme = ColorScheme.of(context);
    return FilledButton(
      onPressed: onPressed,
      style: FilledButton.styleFrom(
        backgroundColor: scheme.surfaceContainerHighest,
        foregroundColor: scheme.onSurface,
        minimumSize: const Size.fromHeight(56),
        padding: const EdgeInsets.symmetric(horizontal: 4, vertical: 8),
        shape: const StadiumBorder(),
      ),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          Icon(icon, size: 22),
          const SizedBox(height: 3),
          // A longer name ("Showcase") shrinks a little rather than being cut.
          FittedBox(
            fit: BoxFit.scaleDown,
            child: Text(
              label,
              maxLines: 1,
              style: const TextStyle(fontSize: 11, fontWeight: FontWeight.w500),
            ),
          ),
        ],
      ),
    );
  }
}

/// A bar button that opens a menu of choices, checking the one that is set,
/// and shows that one as its caption.
class ChoiceMenu<T> extends StatelessWidget {
  const ChoiceMenu({
    super.key,
    required this.title,
    required this.icon,
    required this.choices,
    required this.selected,
    required this.onSelected,
  });

  final String title;
  final IconData icon;
  final List<(String, T)> choices;
  final T selected;
  final ValueChanged<T> onSelected;

  @override
  Widget build(BuildContext context) {
    final current = choices.where((c) => c.$2 == selected).firstOrNull;
    return MenuAnchor(
      menuChildren: [
        Padding(
          padding: const EdgeInsets.fromLTRB(16, 8, 16, 4),
          child: Text(title, style: TextTheme.of(context).labelSmall),
        ),
        for (final (name, value) in choices)
          MenuItemButton(
            leadingIcon: value == selected
                ? const Icon(Icons.check, size: 18)
                : const SizedBox(width: 18),
            onPressed: () => onSelected(value),
            child: Text(name),
          ),
      ],
      builder: (context, controller, _) => BarButton(
        icon: icon,
        label: current?.$1 ?? '—',
        onPressed: () => controller.isOpen ? controller.close() : controller.open(),
      ),
    );
  }
}
