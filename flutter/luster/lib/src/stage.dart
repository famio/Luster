import 'dart:async';
import 'dart:io' as io;
import 'dart:math' as math;
import 'dart:typed_data';
import 'dart:ui' as ui;

import 'package:flutter/foundation.dart' show kIsWeb;
import 'package:flutter_scene/gpu.dart' as gpu;
import 'package:flutter_scene/scene.dart';
import 'package:vector_math/vector_math.dart' as vm;

import 'luster.dart';
import 'rust/api/luster.dart' as rust;

/// The process's environment, where a platform has one.
final Map<String, String> _environment =
    kIsWeb ? const {} : io.Platform.environment;

/// One submesh on stage: its geometry is kept so that re-plating only swaps
/// materials.
class _Part {
  _Part(this.node, this.geometry, this.material);
  final Node node;
  final MeshGeometry geometry;
  final rust.Material material;
}

/// What a badge is drawn in, whoever draws it: the badge in the studio,
/// lit, plated and posed. [LusterView] shows one on screen, and
/// [LusterSnapshot] draws one into a still.
class BadgeStage {
  final Scene scene = Scene();
  Node? _pivot;
  final List<Texture2D> _sheets = [];
  final List<_Part> _parts = [];

  /// What the badge on stage measures across its face, wide by tall.
  double wide = 1, tall = 1;

  /// Counts calls to [light], so that a later one wins.
  int _lit = 0;

  /// The panorama the badge reflects, the showcase's, made once.
  static Future<EnvironmentMap>? _panorama;

  /// The reverse's sandblast, the same for every badge.
  static Future<Texture2D>? _grit;
  Texture2D? _gritTexture;

  /// RealityKit's tone mapping, as a table, made once.
  static Future<ColorLut>? _tone;

  /// The badge's own shader (`shaders/luster_badge.frag`), loaded once; null
  /// where it could not be, and the badge is drawn with flutter_scene's
  /// standard material instead.
  static Future<_BadgeShader?>? _badgeShader;
  _BadgeShader? _shader;

  /// The panorama the badge is lit by, once it is made.
  EnvironmentMap? _environmentMap;

  static final rust.Handling handling = rust.handling();

  /// What the studio's light is worth here: the engine's, set by drawing the
  /// same badge on every platform and matching what came out.
  static final rust.SceneCalibration calibration = rust.sceneCalibration();

  /// `LUSTER_EXPOSURE` in the environment overrides the engine's exposure,
  /// for matching it against the other renderers.
  static double get exposure =>
      double.tryParse(_environment['LUSTER_EXPOSURE'] ?? '') ??
      calibration.exposure;

  /// Loads the engine and what every badge is drawn with, and lights the
  /// studio for [appearance].
  Future<void> setUp(LusterAppearance appearance) async {
    await Luster.init();
    _gritTexture = await (_grit ??= forgetting(_sandblast(), () => _grit = null));
    final tone = await (_tone ??= forgetting(_realityKitTone(), () => _tone = null));
    _shader = await (_badgeShader ??= _BadgeShader.load());
    // flutter_scene's own curves keep a colour's saturation as it
    // brightens, where RealityKit lets it fade towards white, and the
    // pale gold of the Apple side came out orange. With no curve of its
    // own, clamped and read through RealityKit's, the scene shows what
    // RealityKit would.
    scene.toneMapping = ToneMappingMode.linear;
    scene.postProcess.colorGrading.lut = tone;
    await light(appearance);
  }

  /// A shared future that is dropped if it fails, so the next ask tries again.
  static Future<T> forgetting<T>(Future<T> future, void Function() forget) =>
      future.catchError((Object e) {
        forget();
        throw e;
      });

  // MARK: The studio

  static Future<EnvironmentMap> _studio() {
    return _panorama ??= forgetting(() async {
      final panorama = await rust.showcaseEnvironment();
      return EnvironmentMap.fromUIImages(radianceImage: await _image(panorama));
    }(), () => _panorama = null);
  }

  /// Turns a direction in this scene into the one the engine's panorama is
  /// laid out by. The engine's panorama puts longitude at atan2(x, -z) in
  /// its right-handed space, flutter_scene reads one at atan2(z, x), and the
  /// badge arrives here with z negated; together that is x and z swapped.
  /// Without it the studio is reflected mirrored across a diagonal: the
  /// same head on, where it is nearly symmetric, and ever further off as
  /// the badge tips, so a turned badge misses the lights it should catch.
  static final vm.Matrix3 _panoramaAxes = vm.Matrix3(0, 0, 1, 0, 1, 0, 1, 0, 0);

  static Future<ColorLut> _realityKitTone() async =>
      ColorLut.fromCubeString(await rust.realitykitToneCube());

  static Future<Texture2D> _sandblast() async {
    final t = await rust.sandblast();
    // A normal map is data, not colour.
    return Texture2D.fromPixels(t.rgba, t.width, t.height,
        content: TextureContent.normal);
  }

  /// Lights the studio for [appearance]. False when a later call overtook
  /// this one while its panorama was made, and the studio is that one's.
  Future<bool> light(LusterAppearance appearance) async {
    final mine = ++_lit;
    final preset = rust.lightingPreset(lighting: appearance.lighting.engine);
    final environment = await _studio();
    if (mine != _lit) return false;
    scene.environment = environment;
    _environmentMap = environment;
    scene.environmentTransform = _panoramaAxes;
    scene.environmentIntensity = preset.environment;
    scene.exposure = exposure;

    // Filament and RealityKit each take the lamps in their own units; so does
    // this one, which is what the preset's lux and the calibration's lamp
    // unit are for.
    for (final old in scene.root.getComponents<DirectionalLightComponent>().toList()) {
      scene.root.removeComponent(old);
    }
    for (final light in rust.studioLights()) {
      final worth = rust.lamp(role: light.role, lighting: appearance.lighting.engine);
      // A lamp at nothing still costs every pixel its share of the shader:
      // flutter_scene shades each light it is given, lit or not.
      if (worth <= 0) continue;
      // The root has no transform of its own, so the lamp's local direction
      // is where it points in the scene.
      scene.root.addComponent(
        DirectionalLightComponent.aimed(
          DirectionalLight(
            color: vm.Vector3(light.color.r, light.color.g, light.color.b),
            intensity: worth * preset.sceneLux * calibration.lampUnit,
          ),
          vm.Vector3(light.direction[0], light.direction[1], light.direction[2]),
        ),
      );
    }
    return true;
  }

  // MARK: The badge

  /// Puts [badge] on stage in [appearance], at the pose it had; none clears
  /// the stage.
  void show(rust.LusterBadge? badge, LusterAppearance appearance) {
    if (_pivot != null) scene.remove(_pivot!);
    _pivot = null;
    _parts.clear();
    _sheets.clear();
    if (badge == null) return;
    // Every colour the badge wears is on one sheet; the pieces point at it.
    for (final texture in badge.textures) {
      _sheets.add(Texture2D.fromPixels(texture.rgba, texture.width, texture.height));
    }
    final node = Node(name: 'badge');
    wide = 0;
    tall = 0;
    for (final s in badge.submeshes) {
      final geometry = MeshGeometry.fromArrays(
        positions: s.positions,
        normals: s.normals,
        texCoords: s.texCoords,
        tangents: s.tangents,
        indices: s.indices,
      );
      final material = badge.materials[s.material];
      final part = Node(
        name: s.name,
        mesh: Mesh(geometry, _material(material, appearance)),
      );
      _parts.add(_Part(part, geometry, material));
      node.add(part);
      for (var i = 0; i < s.positions.length; i += 3) {
        wide = math.max(wide, s.positions[i].abs() * 2);
        tall = math.max(tall, s.positions[i + 1].abs() * 2);
      }
    }
    scene.add(node);
    _pivot = node;
    pose(_tilt, _spin);
  }

  /// Whether there is a badge on stage.
  bool get showing => _parts.isNotEmpty;

  /// Gives the badge on stage the metal and sheen [appearance] asks for.
  void plate(LusterAppearance appearance) {
    // A new mesh on the same geometry: flutter_scene keeps the node's render
    // items and takes the new material. Assigning a primitive's material
    // alone is not seen, as the items hold the one they were made with.
    for (final part in _parts) {
      part.node.mesh = Mesh(part.geometry, _material(part.material, appearance));
    }
  }

  Material _material(rust.Material m, LusterAppearance appearance) {
    final finish = rust.finish(role: m.role, lighting: appearance.lighting.engine);
    final color = finish.plated ? metalRgba(appearance.metal) : m.color;
    final shader = _shader;
    final environment = _environmentMap;
    // The showcase is lit by its panorama alone, which is all the badge's
    // shader does. The lamps' lighting takes the standard material.
    if (shader != null &&
        environment != null &&
        rust.studioLights().every(
            (light) => rust.lamp(role: light.role, lighting: appearance.lighting.engine) <= 0)) {
      return _badgeMaterial(shader, environment, m, finish, color, appearance);
    }
    final material = PhysicallyBasedMaterial()
      ..baseColorFactor = vm.Vector4(
        _linear(color.r),
        _linear(color.g),
        _linear(color.b),
        1,
      )
      ..metallicFactor = finish.metallic
      ..roughnessFactor = finish.roughness
      ..specular = finish.specular;
    // What the document paints on this face; its colour stands for the sides.
    final index = m.texture;
    if (index != null && index < _sheets.length) {
      material.baseColorTexture = _sheets[index];
    }
    if (m.role == rust.MaterialRole.goldBack && _gritTexture != null) {
      material.normalTexture = _gritTexture;
    }
    return material;
  }

  /// The badge's shader, set up as the standard material would be for the
  /// same finish, colour and textures.
  ShaderMaterial _badgeMaterial(_BadgeShader shader, EnvironmentMap environment, rust.Material m,
      rust.Finish finish, rust.Rgba color, LusterAppearance appearance) {
    final material = ShaderMaterial(
      fragmentShader: shader.flat,
      radianceCubeFragmentShader: shader.cube,
      useEnvironment: true,
    );
    final index = m.texture;
    final normalMap = m.role == rust.MaterialRole.goldBack ? _gritTexture : null;
    // The standard material's dielectric F0 for a specular factor:
    // glass's 4% head on, scaled.
    final f0 = (0.04 * finish.specular).clamp(0.0, 1.0);
    final axes = _panoramaAxes.storage;
    material.setUniformBlockFromFloats('BadgeInfo', [
      _linear(color.r), _linear(color.g), _linear(color.b), 1, //
      f0, f0, f0, 0, //
      finish.metallic, finish.roughness, 1, normalMap != null ? 1 : 0, //
      // The environment's strength, and the standard material's specular
      // anti-aliasing.
      rust.lightingPreset(lighting: appearance.lighting.engine).environment, 0.15, 0.2, 0, //
      axes[0], axes[1], axes[2], 0, //
      axes[3], axes[4], axes[5], 0, //
      axes[6], axes[7], axes[8], 0, //
      0, 0, 0, 1,
    ]);
    material.setTexture('base_color_texture',
        index != null && index < _sheets.length ? _sheets[index] : _BadgeShader.white);
    material.setTexture('normal_texture', normalMap ?? _BadgeShader.white);
    material.setTexture('irradiance_sh', environment.diffuseShTexture);
    return material;
  }

  static double _linear(double c) =>
      c <= 0.04045 ? c / 12.92 : math.pow((c + 0.055) / 1.055, 2.4).toDouble();

  static Future<ui.Image> _image(rust.Texture t) {
    final done = Completer<ui.Image>();
    ui.decodeImageFromPixels(
        t.rgba, t.width, t.height, ui.PixelFormat.rgba8888, done.complete);
    return done.future;
  }

  // MARK: Posing it

  double _tilt = handling.restingTilt, _spin = handling.restingSpin;

  /// Turns the badge to [tilt] (about x) and [spin] (about y), radians. The
  /// engine's meshes arrive mirrored into flutter_scene's left-handed space
  /// (z negated), which reverses the sense of turns about x and y.
  void pose(double tilt, double spin) {
    _tilt = tilt;
    _spin = spin;
    _pivot?.rotation = vm.Quaternion.axisAngle(vm.Vector3(1, 0, 0), -tilt) *
        vm.Quaternion.axisAngle(vm.Vector3(0, 1, 0), -spin);
  }

  /// The studio's camera, wide enough to hold the badge in a view [aspect]
  /// wide over tall: the engine's framing, the one every platform uses.
  Camera camera(double aspect) {
    final setup = rust.studioCamera();
    final fov = rust.fieldOfView(width: wide, height: tall, aspect: aspect);
    return PerspectiveCamera(
      fovRadiansY: fov * math.pi / 180,
      // In front of the face, which looks down -z here.
      position: vm.Vector3(0, 0, -setup.distance),
      target: vm.Vector3.zero(),
    );
  }

  /// The badge drawn into a square [pixelSize] across, as a view draws it,
  /// over [background] or over nothing.
  Future<ui.Image> still(int pixelSize, LusterColor? background) async {
    final size = ui.Size.square(pixelSize.toDouble());
    final recorder = ui.PictureRecorder();
    final canvas = ui.Canvas(recorder);
    if (background != null) {
      canvas.drawRect(
          ui.Offset.zero & size,
          ui.Paint()
            ..color = ui.Color.from(
                alpha: 1, red: background.red, green: background.green, blue: background.blue));
    }
    scene.render(camera(1), canvas, viewport: ui.Offset.zero & size);
    final picture = recorder.endRecording();
    try {
      return await picture.toImage(pixelSize, pixelSize);
    } finally {
      picture.dispose();
    }
  }
}

/// The badge's shader and its twin for a cubemap environment.
class _BadgeShader {
  _BadgeShader(this.flat, this.cube);
  final gpu.Shader flat;
  final gpu.Shader? cube;

  /// Where a material has no texture of its own: the standard material's
  /// white.
  static final Texture2D white =
      Texture2D.fromPixels(Uint8List.fromList([255, 255, 255, 255]), 1, 1);

  static Future<_BadgeShader?> load() async {
    try {
      final library = await gpu.loadShaderLibraryAsync(
          'packages/luster/shaders/generated/luster.shaderbundle');
      final flat = library?['BadgeFragment'];
      if (flat == null) return null;
      return _BadgeShader(flat, library!['BadgeCubeFragment']);
    } catch (_) {
      return null;
    }
  }
}
