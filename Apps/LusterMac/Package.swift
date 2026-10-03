// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "LusterMac",
    platforms: [.macOS(.v15)],
    products: [
        .executable(name: "Luster", targets: ["LusterApp"]),
    ],
    dependencies: [.package(path: "../..")],
    targets: [
        // Reading a drop and naming a file: the app's own business, kept out
        // of the views so it can be tested.
        .target(name: "LusterMacKit"),
        // Named apart from the package it depends on, whose own target is
        // called Luster; the binary is still `Luster`.
        .executableTarget(
            name: "LusterApp",
            dependencies: ["LusterMacKit", .product(name: "LusterUI", package: "Luster")],
            path: "Sources/Luster"),
        .testTarget(name: "LusterMacKitTests", dependencies: ["LusterMacKit"]),
    ]
)
