Pod::Spec.new do |s|
  s.name             = 'luster'
  s.version          = '0.1.0'
  s.summary          = 'Mints SVG artwork into a lit 3D badge.'
  s.description      = <<-DESC
Mints SVG artwork into a lit 3D badge, rendered with flutter_scene. The engine is Luster's Rust core.
                       DESC
  s.homepage         = 'https://github.com/famio/Luster'
  s.license          = { :file => '../LICENSE' }
  s.author           = 'famio'
  s.module_name      = 'luster'

  # Classes holds only an empty C file, so that CocoaPods makes a framework;
  # the engine is linked in below.
  s.source           = { :path => '.' }
  s.source_files = 'Classes/**/*'
  s.dependency 'Flutter'
  s.platform = :ios, '13.0'

  s.swift_version = '5.0'

  s.script_phase = {
    :name => 'Build Rust library',
    # The crate to build, and the library it makes.
    :script => 'sh "$PODS_TARGET_SRCROOT/../cargokit/build_pod.sh" ../rust/crates/luster-dart luster_dart',
    :execution_position => :before_compile,
    :input_files => ['${BUILT_PRODUCTS_DIR}/cargokit_phony'],
    # Let XCode know that the static library referenced in -force_load below is
    # created by this build step.
    :output_files => ["${PODS_CONFIGURATION_BUILD_DIR}/luster/libluster_dart.a"],
  }
  s.pod_target_xcconfig = {
    'DEFINES_MODULE' => 'YES',
    # Flutter.framework does not contain a i386 slice.
    'EXCLUDED_ARCHS[sdk=iphonesimulator*]' => 'i386',
    'OTHER_LDFLAGS' => '-force_load ${PODS_CONFIGURATION_BUILD_DIR}/luster/libluster_dart.a',
  }
end