import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart';
import 'package:flutter_scene/scene.dart';

import 'rust/api/luster.dart' as rust;
import 'rust/frb_generated.dart';

/// Entry points to the engine.
abstract final class Luster {
  static Future<void>? _ready;
  static Future<void>? _engine;
  static Future<void>? _scene;

  /// Loads the Rust engine and flutter_scene's shaders. A [LusterView] and
  /// [mint] call it themselves; calling it early, at launch, gets it done
  /// before the first badge is wanted. Safe to call more than once; later
  /// calls return the first call's future, unless it failed.
  static Future<void> init() => _ready ??= () async {
        try {
          // Each is kept once it has succeeded: the engine may be loaded only
          // once, so a retry after the shaders failed must not load it again.
          await (_engine ??= _retrying(() => RustLib.init(
                // On Apple platforms the pod links the engine statically into
                // the plugin's framework, which the app has already loaded;
                // elsewhere it is its own shared library, found by name.
                externalLibrary: Platform.isIOS || Platform.isMacOS
                    ? ExternalLibrary.process(iKnowHowToUseIt: true)
                    : null,
              ), () => _engine = null));
          await (_scene ??= _retrying(Scene.initializeStaticResources, () => _scene = null));
        } catch (_) {
          // A failure is not kept: the next call tries again.
          _ready = null;
          rethrow;
        }
      }();

  static Future<void> _retrying(Future<void> Function() step, void Function() forget) async {
    try {
      await step();
    } catch (_) {
      forget();
      rethrow;
    }
  }

  /// The engine's version. After [init].
  static String get version => rust.version();

  /// What web sources are fetched with, process-wide: [lusterHttpLoader] unless
  /// the app sets another, such as one over flutter_cache_manager (see the
  /// README).
  static LusterLoader loader = lusterHttpLoader;

  /// Mints a badge from an SVG, on the engine's own threads; a web source is
  /// fetched with [loader] first.
  ///
  /// At most two mints run at once across the process; the rest wait their
  /// turn. Recent badges are kept by their bytes and options, so asking again
  /// is immediate, and asking for one already being minted waits for that
  /// mint rather than starting another. Fails with a [LusterException] when
  /// the document, its source or the engine fails.
  static Future<LusterBadge> mint(LusterSource source,
          {LusterOptions options = const LusterOptions()}) =>
      Minting(source, options).badge();
}

/// One ask for a badge, which can be given up: what [Luster.mint] and a
/// `LusterView` both mint through.
final class Minting {
  Minting(this.source, this.options);

  final LusterSource source;
  final LusterOptions options;
  rust.MintRequest? _request;
  bool _cancelled = false;

  /// Fetches the document if it must, and strikes it on the engine's
  /// threads. Fails with a [LusterException], or with [MintCancelled] once
  /// [cancel] has been called.
  Future<LusterBadge> badge() async {
    final svg = await source.load();
    await Luster.init();
    if (_cancelled) throw const MintCancelled();
    final request = _request = rust.MintRequest(svg: svg, options: options.engine);
    try {
      return LusterBadge._(await request.badge());
    } on rust.LusterError catch (e) {
      if (e.kind == rust.LusterErrorKind.cancelled) throw const MintCancelled();
      throw LusterException._engine(e);
    } finally {
      _request = null;
      request.dispose();
    }
  }

  /// Gives the badge up: the engine stops unless someone else wants it.
  void cancel() {
    _cancelled = true;
    _request?.cancel();
  }
}

/// A [Minting] was given up.
final class MintCancelled implements Exception {
  const MintCancelled();
}

/// Fetches the document a web [LusterSource] names.
typedef LusterLoader = Future<Uint8List> Function(Uri url);

/// Fetches with `dart:io`'s [HttpClient], which keeps nothing.
Future<Uint8List> lusterHttpLoader(Uri url) async {
  final client = HttpClient()
    ..connectionTimeout = const Duration(seconds: 15);
  try {
    final response = await (await client.getUrl(url)).close();
    if (response.statusCode < 200 || response.statusCode > 299) {
      throw LusterException(LusterErrorKind.unreadableSource,
          'could not fetch $url: HTTP ${response.statusCode}');
    }
    if (response.contentLength > LusterSource.maximumBytes) {
      throw LusterException.tooLarge(response.contentLength);
    }
    final bytes = BytesBuilder(copy: false);
    await for (final chunk in response) {
      bytes.add(chunk);
      if (bytes.length > LusterSource.maximumBytes) {
        throw LusterException.tooLarge(bytes.length);
      }
    }
    return bytes.takeBytes();
  } finally {
    client.close(force: true);
  }
}

/// Where a badge's SVG comes from: its bytes, its text, or a web URL fetched
/// with [Luster.loader].
final class LusterSource {
  const LusterSource._(this._bytes, this.url);

  /// SVG document bytes.
  factory LusterSource.bytes(Uint8List bytes) => LusterSource._(bytes, null);

  /// SVG document text.
  factory LusterSource.svg(String text) =>
      LusterSource._(Uint8List.fromList(utf8.encode(text)), null);

  /// An http(s) URL, fetched with [Luster.loader].
  factory LusterSource.url(Uri url) {
    if (url.scheme != 'https' && url.scheme != 'http') {
      throw ArgumentError.value(url, 'url', 'a web source is http(s)');
    }
    return LusterSource._(null, url);
  }

  final Uint8List? _bytes;

  /// The URL to fetch, for a web source.
  final Uri? url;

  /// Larger documents are refused before they are read in full.
  static const maximumBytes = 20 << 20;

  /// The SVG bytes, fetched for a web source.
  Future<Uint8List> load() async {
    final bytes = _bytes;
    if (bytes != null) {
      if (bytes.length > maximumBytes) throw LusterException.tooLarge(bytes.length);
      return bytes;
    }
    final Uint8List data;
    try {
      data = await Luster.loader(url!);
    } on LusterException {
      rethrow;
    } catch (e) {
      throw LusterException(LusterErrorKind.unreadableSource, 'could not fetch $url: $e');
    }
    if (data.length > maximumBytes) throw LusterException.tooLarge(data.length);
    return data;
  }

  @override
  bool operator ==(Object other) =>
      other is LusterSource &&
      other.url == url &&
      (identical(other._bytes, _bytes) ||
          (other._bytes != null && _bytes != null && _sameBytes(other._bytes, _bytes)));

  @override
  int get hashCode => url?.hashCode ?? Object.hash(_bytes!.length, _bytes.isEmpty ? 0 : _bytes.first);

  @override
  String toString() => url != null ? 'LusterSource($url)' : 'LusterSource(${_bytes!.length} bytes)';
}

bool _sameBytes(Uint8List a, Uint8List b) {
  if (a.length != b.length) return false;
  for (var i = 0; i < a.length; i++) {
    if (a[i] != b[i]) return false;
  }
  return true;
}

/// What went wrong, as [LusterException.kind] tells it.
enum LusterErrorKind {
  /// The data is not an SVG document.
  invalidSvg,

  /// The SVG exceeds the engine's limits (shapes, path segments, …).
  inputTooComplex,

  /// The SVG draws nothing a badge can be made of.
  nothingToMint,

  /// The source is larger than [LusterSource.maximumBytes].
  tooLarge,

  /// The URL could not be fetched.
  unreadableSource,

  /// The engine itself went wrong: a bug, not the document's doing.
  engineFailure,
}

/// Why a badge could not be made: the document, its source, or the engine.
final class LusterException implements Exception {
  const LusterException(this.kind, this.message);

  LusterException.tooLarge(int size)
      : kind = LusterErrorKind.tooLarge,
        message = 'the document is $size bytes; at most ${LusterSource.maximumBytes} are read';

  LusterException._engine(rust.LusterError e)
      : kind = switch (e.kind) {
          rust.LusterErrorKind.invalidSvg => LusterErrorKind.invalidSvg,
          rust.LusterErrorKind.inputTooComplex => LusterErrorKind.inputTooComplex,
          rust.LusterErrorKind.nothingToMint => LusterErrorKind.nothingToMint,
          // Minting turns the engine's cancellation into MintCancelled.
          rust.LusterErrorKind.cancelled ||
          rust.LusterErrorKind.internal =>
            LusterErrorKind.engineFailure,
        },
        message = e.reason;

  final LusterErrorKind kind;

  /// What went wrong, for a person to read.
  final String message;

  @override
  String toString() => 'LusterException(${kind.name}): $message';
}

/// A colour, sRGB 0…1: the metal a badge is plated in.
final class LusterColor {
  const LusterColor(this.red, this.green, this.blue);

  final double red;
  final double green;
  final double blue;

  /// The engine's default metal: pale gold. After [Luster.init].
  static final LusterColor gold = LusterColor._of(rust.defaultGold());
  static final LusterColor silver = LusterColor._of(rust.silver());
  static final LusterColor copper = LusterColor._of(rust.copper());

  LusterColor._of(rust.Rgba c) : this(c.r, c.g, c.b);

  @override
  bool operator ==(Object other) =>
      other is LusterColor && other.red == red && other.green == green && other.blue == blue;

  @override
  int get hashCode => Object.hash(red, green, blue);

  @override
  String toString() => 'LusterColor($red, $green, $blue)';
}

/// The engine's colour for [color]; gold for none.
rust.Rgba metalRgba(LusterColor? color) => color == null
    ? rust.defaultGold()
    : rust.Rgba(r: color.red, g: color.green, b: color.blue, a: 1);

/// How a badge is lit.
enum LusterLighting {
  /// A product photographer's studio: strip lights that sweep across the
  /// enamel as a band of sheen when the badge turns, over a dim tent that
  /// keeps plated lines their colour at any angle, and no lamps. The enamel
  /// shows the colours the document paints.
  showcase,

  /// For checking a badge's shape: lamps alone, no reflections, and nothing
  /// that shines.
  off;

  /// The engine's name for it.
  rust.Lighting get engine => switch (this) {
        showcase => rust.Lighting.showcase,
        off => rust.Lighting.off,
      };
}

/// What to strike. Changing any of it mints the badge again.
///
/// A badge is the art's silhouette in metal, its fills set into it as enamel
/// that runs on to the edge, recessed below the metal its lines stand in.
final class LusterOptions {
  const LusterOptions({
    this.metalLines = false,
    this.withoutHiddenFaces = false,
  });

  /// Leave out the faces no view can see. Worth it for a badge being sent
  /// somewhere, not for one on screen.
  final bool withoutHiddenFaces;

  /// Leave the lines and the edge's top in the metal. By default they are
  /// plated in the colours the document paints on them, each line in its
  /// stroke colour.
  final bool metalLines;

  rust.MintOptions get engine =>
      rust.MintOptions(withoutHiddenFaces: withoutHiddenFaces, metalLines: metalLines);

  @override
  bool operator ==(Object other) =>
      other is LusterOptions &&
      other.withoutHiddenFaces == withoutHiddenFaces &&
      other.metalLines == metalLines;

  @override
  int get hashCode => Object.hash(withoutHiddenFaces, metalLines);
}

/// How the badge is lit and plated. Changing it re-lights and re-plates; it
/// never rebuilds.
final class LusterAppearance {
  const LusterAppearance({
    this.metal,
    this.lighting = LusterLighting.showcase,
  });

  /// The metal's colour; null takes the engine's pale gold.
  final LusterColor? metal;
  final LusterLighting lighting;

  @override
  bool operator ==(Object other) =>
      other is LusterAppearance && other.metal == metal && other.lighting == lighting;

  @override
  int get hashCode => Object.hash(metal, lighting);
}

/// A struck badge: what a view shows, and the bytes to send one somewhere
/// else.
final class LusterBadge {
  LusterBadge._(this.engine);

  /// What the engine handed over, for the view to build its scene from.
  final rust.LusterBadge engine;

  /// Identifies the badge: the document and the options it was struck with.
  /// Equal keys mean the same badge.
  String get designKey => engine.designKey;

  /// The badge as a glTF binary: meshes, materials and textures in one file,
  /// its metal plated in [metal] (null takes the engine's pale gold), [scale]
  /// badge-widths across. The same badge always writes the same bytes. Runs
  /// on Rust's threads.
  Future<Uint8List> glb({double scale = 1, LusterColor? metal}) =>
      engine.struck.glb(scale: scale, metal: metalRgba(metal));

  @override
  bool operator ==(Object other) => other is LusterBadge && other.designKey == designKey;

  @override
  int get hashCode => designKey.hashCode;

  @override
  String toString() => 'LusterBadge($designKey)';
}

/// What a badge view is doing.
sealed class LusterState {
  const LusterState();
}

/// No document yet.
final class LusterIdle extends LusterState {
  const LusterIdle();
}

/// Minting a document; the previous badge, if any, stays on screen.
final class LusterMinting extends LusterState {
  const LusterMinting();
}

final class LusterReady extends LusterState {
  const LusterReady(this.badge);
  final LusterBadge badge;
}

final class LusterFailed extends LusterState {
  const LusterFailed(this.error);

  /// Usually a [LusterException], whose `kind` says what went wrong.
  final Object error;
}
