// swift-tools-version: 6.0
import PackageDescription

// The engine is Rust (rust/), linked as a static XCFramework. During
// development that is the one Scripts/build-xcframework.sh leaves in
// rust/target/apple; a tagged release points at the zip attached to its
// GitHub release instead (Scripts/release.sh rewrites this target).
let ffi: Target = .binaryTarget(
    name: "LusterFFI",
    url: "https://github.com/famio/Luster/releases/download/0.1.0/LusterFFI.xcframework.zip",
    checksum: "f7b176eddf3c0e28454e426fc36d2d78ff05d27d8911baca919d497367c34264"
)

let package = Package(
    name: "Luster",
    platforms: [.iOS(.v18), .macOS(.v15)],
    products: [
        .library(name: "Luster", targets: ["Luster"]),
        .library(name: "LusterUI", targets: ["LusterUI"]),
    ],
    targets: [
        ffi,
        // The Swift half of the uniffi bindings, generated; not public API.
        .target(name: "LusterCore", dependencies: ["LusterFFI"],
                path: "Sources/LusterCore"),
        .target(name: "Luster", dependencies: ["LusterCore"]),
        .target(name: "LusterUI", dependencies: ["Luster"]),
        .testTarget(name: "LusterTests", dependencies: ["Luster"]),
        .testTarget(name: "LusterUITests", dependencies: ["LusterUI"]),
    ]
)
