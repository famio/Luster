// swift-tools-version: 5.9
import PackageDescription

// The engine is Rust (rust/crates/luster-dart), linked as a dynamic
// XCFramework. During development that is the one
// Scripts/build-flutter-xcframework.sh leaves beside this file; a tagged
// release points at the zip attached to its GitHub release instead
// (Scripts/release.sh rewrites this target).
let engine: Target = .binaryTarget(
    name: "luster_dart",
    url: "https://github.com/famio/Luster/releases/download/0.3.0/luster_dart.xcframework.zip",
    checksum: "549695f3f45783fdb2f6d36c2b8e8c9fb448df7fc39965b0f843810d3ba51786"
)

let package = Package(
    name: "luster",
    platforms: [.iOS("13.0"), .macOS("10.15")],
    products: [
        .library(name: "luster", targets: ["luster"])
    ],
    // Flutter asks every plugin for this, though nothing here uses it.
    dependencies: [
        .package(name: "FlutterFramework", path: "../FlutterFramework")
    ],
    targets: [
        engine,
        // Dart calls the engine through FFI, so this has no code of its own:
        // it is what brings the engine into the app.
        .target(
            name: "luster",
            dependencies: [
                "luster_dart",
                .product(name: "FlutterFramework", package: "FlutterFramework"),
            ]),
    ]
)
