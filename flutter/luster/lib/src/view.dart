import 'dart:async';
import 'dart:math' as math;
import 'dart:ui' as ui;

import 'package:flutter/scheduler.dart' show Ticker;
import 'package:flutter/widgets.dart';
import 'package:flutter_scene/scene.dart';

import 'luster.dart';
import 'rust/api/luster.dart' as rust;
import 'stage.dart';

/// A badge minted from an SVG, lit in the studio. Drag to turn it; it keeps
/// spinning briefly with momentum, unless the system asks for reduced motion.
///
/// The engine runs on its own threads, so minting never blocks the UI
/// isolate. A different [source] gives up a mint in flight; an equal one does
/// nothing.
///
/// Until a badge is on screen the widget shows its [placeholder], if it is
/// given one:
///
/// ```dart
/// LusterView(
///   source: source,
///   placeholder: (context, state) => state is LusterFailed
///       ? const Icon(Icons.error_outline)
///       : const CircularProgressIndicator(),
/// )
/// ```
class LusterView extends StatefulWidget {
  const LusterView({
    super.key,
    required this.source,
    this.options = const LusterOptions(),
    this.appearance = const LusterAppearance(),
    this.momentumEnabled = true,
    this.onStateChange,
    this.placeholder,
  });

  final LusterSource? source;
  final LusterOptions options;
  final LusterAppearance appearance;

  /// Whether a flick keeps the badge turning; never while the system asks
  /// for reduced motion.
  final bool momentumEnabled;

  /// Told of every change: minting, ready with a badge, or failed.
  final ValueChanged<LusterState>? onStateChange;

  /// Shown over the view, in the middle, while a source is set and no badge
  /// is on screen: while the first is struck, given [LusterMinting], and if
  /// it fails, given [LusterFailed]. A badge struck later takes the place of
  /// the one on screen without it.
  final Widget Function(BuildContext context, LusterState state)? placeholder;

  @override
  State<LusterView> createState() => _LusterViewState();
}

class _LusterViewState extends State<LusterView>
    with SingleTickerProviderStateMixin {
  final BadgeStage stage = BadgeStage();
  Scene get scene => stage.scene;
  bool ready = false;
  /// Whether the engine is loaded, so that the studio can be asked for.
  bool started = false;
  Minting? minting;
  int generation = 0;

  /// The last failure, with the source and options it was for: the
  /// placeholder is told of it only while they are the widget's.
  (LusterFailed, LusterSource?, LusterOptions)? failure;

  /// Why the engine would not start, if it would not: nothing is minted
  /// after, and the placeholder is told of it whatever the source.
  LusterFailed? broken;

  /// Whether the badge on screen has been drawn, so that the placeholder
  /// can go. See [_settle].
  bool settled = false;

  /// Moves on whenever the stage is emptied, giving up a wait for a badge
  /// shown before to be drawn.
  int emptied = 0;

  static final rust.Handling handling = BadgeStage.handling;

  // Tilt (about x) and spin (about y), radians.
  double tilt = handling.restingTilt, spin = handling.restingSpin;
  double velocity = 0;
  bool dragging = false;

  @override
  void initState() {
    super.initState();
    _setUp();
  }

  Future<void> _setUp() async {
    try {
      await Luster.init();
      started = true;
      await stage.setUp(widget.appearance);
    } catch (e) {
      if (!mounted) return;
      final failed = LusterFailed(e);
      setState(() => broken = failed);
      widget.onStateChange?.call(failed);
      return;
    }
    if (!mounted) return;
    setState(() => ready = true);
    await _load(widget.source);
  }

  @override
  void didUpdateWidget(LusterView old) {
    super.didUpdateWidget(old);
    // Deferred: _load reports state, and a parent that rebuilds on state
    // must not be told while it is still building.
    if (old.source != widget.source || old.options != widget.options) {
      Future.microtask(() => _load(widget.source));
    }
    if (old.appearance != widget.appearance && started) {
      stage.plate(widget.appearance);
      if (stage.showing) setState(() {});
      stage.light(widget.appearance).then((lit) {
        if (lit && mounted) setState(() {});
      }, onError: (Object e) {
        // The lighting's failure, not the badge's: the placeholder is not
        // told.
        if (mounted) widget.onStateChange?.call(LusterFailed(e));
      });
    }
  }

  @override
  void dispose() {
    _frames.dispose();
    _drawn.dispose();
    _giveUp();
    super.dispose();
  }

  // MARK: The badge

  /// Gives up the mint in flight, if any: the engine stops unless someone
  /// else wants the same badge.
  void _giveUp() {
    minting?.cancel();
    minting = null;
  }

  Future<void> _load(LusterSource? source) async {
    // Before the engine is up there is nothing to ask; `_setUp` loads the
    // document the widget has by then. One that would not start is never
    // asked.
    if (!mounted || !started || broken != null) return;
    _giveUp();
    final mine = ++generation;
    if (source == null) {
      stage.show(null, widget.appearance);
      settled = false;
      emptied++;
      setState(() {});
      _report(const LusterIdle(), source);
      return;
    }
    _report(const LusterMinting(), source);
    final minting = this.minting = Minting(source, widget.options);
    try {
      final badge = await minting.badge();
      if (!mounted || mine != generation) return;
      stage.show(badge.engine, widget.appearance);
      setState(() {});
      _report(LusterReady(badge), source);
      if (!settled) unawaited(_settle(emptied));
      // Once it is on screen, get ready to turn it.
      WidgetsBinding.instance.addPostFrameCallback((_) => _prepareToTurn(mine));
    } catch (e) {
      // A newer document took over; it reports its own state.
      if (mounted && mine == generation) _report(LusterFailed(e), source);
    }
  }

  /// Tells the parent of [state], and keeps a failure for the placeholder,
  /// with the [source] that failed.
  void _report(LusterState state, LusterSource? source) {
    final failure = state is LusterFailed ? (state, source, widget.options) : null;
    if (failure != null || this.failure != null) setState(() => this.failure = failure);
    widget.onStateChange?.call(state);
  }

  /// Lets the placeholder go once the first badge has been drawn. The
  /// frame that first draws it can take a third of a second to rasterize,
  /// and a fade timed from that frame would be over by the next. The frames
  /// after it wait for the raster thread to catch up, so the fade starts
  /// three frames on. A newer source does not stop it: the badge stays on
  /// screen while the next is struck.
  Future<void> _settle(int mine) async {
    for (var frame = 0; frame < 3; frame++) {
      await WidgetsBinding.instance.endOfFrame;
      if (!mounted || mine != emptied) return;
    }
    setState(() => settled = true);
  }

  /// What the placeholder is told: the failure of the widget's source and
  /// options, and minting until then, before their mint has begun too.
  LusterState get _placeholderState => switch ((broken, failure)) {
        (final broken?, _) => broken,
        (_, (final failed, final source, final options))
            when source == widget.source && options == widget.options =>
          failed,
        _ => const LusterMinting(),
      };

  /// [view] under the placeholder, if the widget has one. The same with or
  /// without, so that giving the widget one keeps the view.
  Widget _withPlaceholder(BuildContext context, Widget view) {
    final placeholder = widget.placeholder;
    final shown = widget.source != null && !(stage.showing && settled);
    return Stack(
      fit: StackFit.passthrough,
      children: [
        view,
        if (placeholder != null)
          Positioned.fill(
            child: AnimatedSwitcher(
              // There from the first frame, and fading once the badge has been
              // drawn.
              duration: Duration.zero,
              reverseDuration: const Duration(milliseconds: 200),
              switchOutCurve: Curves.easeOut,
              // A placeholder on its way out leaves a touch to the badge. Every
              // child is wrapped alike, under its own key, so that each keeps
              // its state as the others come and go.
              layoutBuilder: (current, previous) => Stack(
                alignment: Alignment.center,
                children: [
                  for (final child in [...previous, ?current])
                    IgnorePointer(
                      key: child.key,
                      ignoring: !identical(child, current),
                      child: child,
                    ),
                ],
              ),
              child: shown
                  // Its own layer, so that a spinner turning over the view
                  // repaints only itself.
                  ? RepaintBoundary(
                      key: const ValueKey('placeholder'),
                      child: Center(child: placeholder(context, _placeholderState)),
                    )
                  : const SizedBox.shrink(),
            ),
          ),
      ],
    );
  }

  // MARK: Turning it

  void _pose() => stage.pose(tilt, spin);

  /// Draws the badge every frame while it turns. One ticker, started and
  /// stopped: SceneView's `autoTick` makes a new one each time it is turned
  /// on, which its single-ticker state does not allow, and a debug build
  /// shows a red error screen for the frame a turn starts.
  late final Ticker _frames = createTicker(_frame);
  Duration? _lastFrame;

  /// Counts the frames drawn while the badge turns. Only the SceneView
  /// listens: it draws again whenever it is rebuilt, and rebuilding the
  /// rest of the view every frame slows a debug build down until a flick's
  /// speed is lost.
  final ValueNotifier<int> _drawn = ValueNotifier(0);

  /// Starts drawing every frame, if it is not already.
  void _turnOn() {
    if (!_frames.isActive) _frames.start();
  }

  void _frame(Duration elapsed) {
    final last = _lastFrame;
    _lastFrame = elapsed;
    _tick(last == null ? 0 : (elapsed - last).inMicroseconds / 1e6);
    if (!_moving) {
      // At rest: stop drawing every frame, and draw the last pose at full
      // resolution.
      _frames.stop();
      _lastFrame = null;
      setState(() {});
      return;
    }
    _drawn.value++;
  }

  void _tick(double dt) {
    _pace(dt);
    if (dragging || velocity == 0) return;
    final coast = rust.coast(velocity: velocity, dt: dt);
    spin += coast.turn;
    velocity = coast.velocity;
    _pose();
  }

  /// How finely the badge is drawn while it turns, as a part of the
  /// display's resolution. It starts where the GPU should keep up
  /// (`_prepareToTurn`), comes down further while frames come too slowly,
  /// and is kept for the next turn; at rest the badge is drawn full again.
  /// flutter_scene's lit shader costs a fixed amount per pixel, and a phone
  /// GPU facing a badge that fills the screen can take a hundred
  /// milliseconds a frame at full resolution and ten at half — past a size
  /// the frame no longer fits the GPU's tile memory. A fast GPU keeps up at
  /// full resolution and is never touched.
  double _turningScale = 1;

  /// What a frame at full resolution costs this device's GPU, in seconds a
  /// million pixels: measured once a process, on the first badge shown.
  static Future<double>? _gpuCost;

  /// What the view measures and how it is seen, as last built: what a
  /// frame drawn away from the screen has to match.
  Size _viewSize = Size.zero;
  Camera? _camera;

  /// Sets the resolution the badge starts turning at, and makes what turning
  /// at it takes, before the first turn. Otherwise the first turn starts at
  /// full resolution: on a phone GPU its first frames take a tenth of a
  /// second, the touches of a flick arrive too far apart for its speed to
  /// be read, and the badge stops dead under the finger; and the coarser
  /// drawing it then falls back to makes its shaders mid-turn, another
  /// tenth of a second.
  Future<void> _prepareToTurn(int mine) async {
    final camera = _camera;
    if (!mounted || mine != generation || camera == null || _viewSize.isEmpty) return;
    final ratio = MediaQuery.devicePixelRatioOf(context);
    final pixels = _viewSize.width * _viewSize.height * ratio * ratio / 1e6;
    try {
      final cost = await (_gpuCost ??= BadgeStage.forgetting(
          _measureGpu(camera, _viewSize, ratio, pixels), () => _gpuCost = null));
      if (!mounted || mine != generation) return;
      // The scale whose pixels fit a frame, as `_pace` would step to.
      final keepsUp = math.sqrt(_slowFrame / (cost * pixels)) * 0.95;
      _turningScale = math.min(_turningScale, keepsUp.clamp(_coarsest, 1.0));
      if (_turningScale < 1 && !_moving) {
        await scene.warmUp([
          RenderView(
            camera: camera,
            antiAliasingMode: AntiAliasingMode.fxaa,
            renderScale: _turningScale,
          ),
        ]);
      }
    } catch (_) {
      // Only a head start: without it the badge still turns, and `_pace`
      // brings it down.
    }
  }

  /// Draws the scene as the view does, away from the screen, and waits for
  /// the GPU to finish: the quickest of a few tries is what a frame costs.
  /// The first pays for anything not yet made, and a GPU that has been idle
  /// takes a frame or two to get up to speed.
  Future<double> _measureGpu(Camera camera, Size size, double ratio, double pixels) async {
    Future<double> frame() async {
      final watch = Stopwatch()..start();
      final recorder = ui.PictureRecorder();
      scene.render(camera, ui.Canvas(recorder), viewport: Offset.zero & size, pixelRatio: ratio);
      final picture = recorder.endRecording();
      final image = await picture.toImage(1, 1);
      // Reading it back is what waits for the GPU.
      await image.toByteData();
      image.dispose();
      picture.dispose();
      return watch.elapsedMicroseconds / 1e6;
    }

    await frame();
    var quickest = double.infinity;
    for (var i = 0; i < 3; i++) {
      quickest = math.min(quickest, await frame());
    }
    return quickest / pixels;
  }

  /// The frames of this turn so far, and how long they are taking.
  int _turned = 0;
  double _frameTime = 0;

  /// The slowest a frame may take while the badge turns before it is drawn
  /// coarser: 45 frames a second.
  static const _slowFrame = 1 / 45;

  /// The coarsest the badge is drawn while turning.
  static const _coarsest = 0.4;

  void _pace(double dt) {
    // The first frames of a turn are the ones that change resolution.
    if (++_turned <= 3) return;
    _frameTime = _frameTime == 0 ? dt : _frameTime * 0.7 + dt * 0.3;
    if (_turned % 6 == 0 && _frameTime > _slowFrame && _turningScale > _coarsest) {
      // A frame's cost goes with the pixels drawn, the square of the scale:
      // step straight to the scale that should keep up, and a little under.
      final keepsUp = _turningScale * math.sqrt(_slowFrame / _frameTime) * 0.95;
      // At least a tenth down, never below the coarsest. Not `clamp`: just
      // above the coarsest, a tenth down is below it, and `clamp` throws
      // when its bounds cross; thrown from a frame, that leaves the ticker
      // stopped for good, and the badge never coasts again.
      _turningScale = math.max(_coarsest, math.min(keepsUp, _turningScale * 0.9));
      scene.renderScale = _turningScale;
      scene.antiAliasingMode = AntiAliasingMode.fxaa;
      _turned = 0;
      _frameTime = 0;
    }
  }

  /// Whether the badge is turning, under a finger or on its own. Only then
  /// is the scene drawn every frame; at rest it is drawn when something
  /// changes. Drawing a still badge sixty times a second keeps the GPU
  /// flat out for nothing, heats a phone until it slows down, and holds up
  /// the UI thread while each frame waits for the last.
  bool get _moving => dragging || velocity != 0;

  @override
  Widget build(BuildContext context) {
    final still = MediaQuery.maybeDisableAnimationsOf(context) ?? false;
    final moving = _moving;
    if (!moving) _turned = 0;
    scene.renderScale = moving ? _turningScale : 1;
    // Drawn coarser, it is smoothed after the fact rather than sampled four
    // times over: in motion no one sees the difference, and it is the
    // cheaper of the two.
    scene.antiAliasingMode =
        moving && _turningScale < 1 ? AntiAliasingMode.fxaa : AntiAliasingMode.auto;
    if (!ready) return _withPlaceholder(context, const SizedBox.expand());
    final view = GestureDetector(
      onPanStart: (_) {
        setState(() {
          dragging = true;
          velocity = 0;
        });
        _turnOn();
      },
      onPanUpdate: (d) {
        spin += d.delta.dx * handling.spinPerPoint;
        tilt = (tilt + d.delta.dy * handling.tiltPerPoint)
            .clamp(-handling.tiltLimit, handling.tiltLimit);
        _pose();
      },
      onPanCancel: () => setState(() => dragging = false),
      onPanEnd: (d) => setState(() {
        dragging = false;
        velocity = still || !widget.momentumEnabled
            ? 0
            : d.velocity.pixelsPerSecond.dx * handling.spinPerPoint;
      }),
      child: LayoutBuilder(
        builder: (context, box) {
          final camera = stage.camera(box.maxWidth / math.max(box.maxHeight, 1));
          _viewSize = box.biggest;
          _camera = camera;
          return ValueListenableBuilder<int>(
            valueListenable: _drawn,
            builder: (context, frame, child) => SceneView(
              scene,
              camera: camera,
              // Drawn every frame by `_frames` while it turns, not by
              // SceneView's own ticker.
              autoTick: false,
            ),
          );
        },
      ),
    );
    return _withPlaceholder(context, view);
  }
}
