// swift-tools-version: 6.0
import PackageDescription

// A headless renderer for the Rust engine, so a badge can be looked at without
// opening a window. A tool for working on the engine, not an example.
let package = Package(
    name: "LusterShot",
    platforms: [.macOS(.v15)],
    dependencies: [.package(path: "../..")],
    targets: [
        .executableTarget(name: "LusterShot", dependencies: [
            .product(name: "Luster", package: "Luster"),
        ]),
    ]
)
