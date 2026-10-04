import 'dart:io';
import 'dart:isolate';

import 'package:flutter_gpu_shaders/build.dart';
import 'package:hooks/hooks.dart';

/// Compiles the badge's shader against the Flutter SDK building the app, as
/// flutter_scene compiles its own: a shader bundle is only good for the
/// engine that reads it. The shader includes flutter_scene's GLSL, so its
/// shaders directory is on the include path.
void main(List<String> args) async {
  await build(args, (input, output) async {
    final scene = await Isolate.resolvePackageUri(
        Uri.parse('package:flutter_scene/build_hooks.dart'));
    if (scene == null) {
      throw Exception('luster: could not find the flutter_scene package.');
    }
    await buildShaderBundleJson(
      buildInput: input,
      buildOutput: output,
      manifestFileName: 'shaders/luster.shaderbundle.json',
      includeDirectories: [
        scene.resolve('../shaders/'),
        input.packageRoot.resolve('shaders/'),
      ],
      // dFdx and texelFetch, as flutter_scene's own shaders.
      glesLanguageVersion: 300,
    );
    // The bundle lands in build/, which a published package leaves out; the
    // pubspec lists a directory that ships instead, and this fills it.
    File.fromUri(input.packageRoot
            .resolve('build/shaderbundles/luster.shaderbundle'))
        .copySync(input.packageRoot
            .resolve('shaders/generated/luster.shaderbundle')
            .toFilePath());
  });
}
