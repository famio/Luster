import 'dart:ui' as ui;

import 'luster.dart';
import 'stage.dart';

/// Renders a badge to a still image, without a view.
///
/// For thumbnails in a list, where a live view of each badge would cost far
/// more than the picture is worth. A still is drawn as [LusterView] draws, so
/// it is the picture a view shows.
abstract final class LusterSnapshot {
  /// A square still of [badge] at [tilt] and [spin] (radians; the resting
  /// pose when null), [pixelSize] across, over [background], or with nothing
  /// behind it when that is null: the image's alpha is then the badge's
  /// coverage.
  static Future<ui.Image> image(
    LusterBadge badge, {
    LusterAppearance appearance = const LusterAppearance(),
    double? tilt,
    double? spin,
    int pixelSize = 768,
    LusterColor? background,
  }) async {
    if (pixelSize <= 0) throw ArgumentError.value(pixelSize, 'pixelSize', 'a still needs pixels');
    final stage = BadgeStage();
    await stage.setUp(appearance);
    stage.show(badge.engine, appearance);
    stage.pose(tilt ?? BadgeStage.handling.restingTilt, spin ?? BadgeStage.handling.restingSpin);
    return stage.still(pixelSize, background);
  }

  /// The same, for the badge [source] strikes with [options].
  static Future<ui.Image> fromSource(
    LusterSource source, {
    LusterOptions options = const LusterOptions(),
    LusterAppearance appearance = const LusterAppearance(),
    double? tilt,
    double? spin,
    int pixelSize = 768,
    LusterColor? background,
  }) async =>
      image(await Luster.mint(source, options: options),
          appearance: appearance, tilt: tilt, spin: spin, pixelSize: pixelSize,
          background: background);
}
