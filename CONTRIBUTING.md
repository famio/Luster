# Contributing

## Building from a checkout

Building the engine needs Rust (`rust/rust-toolchain.toml` pins the version and the
targets), and for Android the NDK and `cargo-ndk`.

```sh
Scripts/build-xcframework.sh   # iOS, the simulator and macOS; regenerates the Swift bindings
Scripts/build-android.sh       # arm64-v8a and x86_64; regenerates the Kotlin bindings
```

The first leaves the XCFramework `Package.swift` points at; the second leaves the `.so`
files in `android/luster`. The Flutter package builds the engine itself when the app is
built, and pins its Flutter version in `flutter/luster/.fvmrc`.

To use the Android libraries in an app before they are published, put them in the local
Maven repository after `Scripts/build-android.sh`:

```sh
cd android && ./gradlew :luster:publishToMavenLocal :luster-compose:publishToMavenLocal
```

and in the app, add `mavenLocal()` to the repositories and depend on
`dev.famio:luster:0.1.0` (and `dev.famio:luster-compose:0.1.0` for Compose).

## Checks

```sh
cargo test --manifest-path rust/Cargo.toml   # the engine
swift test                                   # the Swift package
Scripts/parity.sh                            # what the engine makes of the fixtures
```

`fixtures/svg` holds the documents the engine is tested on, and `fixtures/golden` what
it must make of them: the regions it reads, the cells before and after merging, the
pieces struck with the lines plated and left in the metal, with their areas, bounds and
colours, and a hash of every GLB it writes. Anything that moves in one of them is
reported, down to a byte of a badge. CI strikes the same documents on Linux, on both
architectures, against the same hashes: a badge has to be the same bytes wherever it is
made. That is why the engine's maths goes through `libm` rather than the platform's own
(`luster-core/src/math.rs`; clippy refuses the std functions). `luster golden <dir>
<svg>…` writes the dumps, for a change meant to move them or a document added later.

Three of the documents are heavier than the rest on purpose — a scene under gradients,
a wheel of thirty flat colours under a wash, and a wreath of small pieces with thin line
work — because a badge someone actually draws is nothing like a test shape.

## Demos and tools

**`Examples/LusterDemo`** (iOS) takes these launch arguments:

| | |
| --- | --- |
| `-tab uikit` | Open on the UIKit tab |
| `-url <url>` | Open on a document on the web |
| `-cycle` | Measure minting and the longest main-thread frame gap |
| `-compare` | Set `LusterView` above `LusterUIView`, to see that they draw alike |

**`android/sample`** takes these intent extras:

| | |
| --- | --- |
| `--es url <url>` | Open on a document on the web (a debug build may fetch plain HTTP from the development machine, 10.0.2.2) |
| `--ez bare true --es tab view` | Show one tab's badge alone, to compare stills |
| `--ez list true` | Set badges in a scrolling list; with `--ez stills true`, stills (`LusterSnapshot`) instead |

**`flutter/luster/example`** opens on a document on the web with
`--dart-define=LUSTER_URL=<url>`.

**`Tools/LusterShot`** is a headless renderer:

```sh
LusterShot out.png art.svg [--metal-lines] [--off] [--background hex|clear] [--size n] [--spin deg] [--tilt deg]
LusterShot --tone-table out.bin   # measures RealityKit's tone mapping for the Flutter view
```

`LUSTER_TIME=1` prints where a scene's time goes.

## Project layout

- `Package.swift`, `Sources/Luster`, `Sources/LusterUI` — the Swift package. `Luster`
  mints and takes stills; `LusterUI` has the SwiftUI, UIKit and AppKit views.
  `Sources/LusterCore/Generated` is the uniffi output, committed.
- `rust/crates/luster-core` — the engine, in the order a badge is made: `svg` (reading
  and cells), `design` (artwork), `mint` (the plate, the badge, its metal), `mesh`
  (extrusion), `cull` (hidden faces), `badge` (assembly) and `export` (GLB). Beneath
  them `geom` and `raster` (paths, booleans, distance fields, tracing), `paint` (face
  colours), `texture`, and `model` and `math`, which use nothing else. `style` is the
  studio every renderer builds: the lighting, how each material is finished, the
  framing and how a drag turns the badge, and what each renderer's numbers are worth,
  RealityKit's measured tone mapping among them, so the three platforms cannot drift
  apart. `mints` is how every binding asks for a badge: a few at once, the recent ones
  kept, and one mint shared by everyone asking for the same one
- `rust/crates/luster-ffi` — uniffi bindings for Swift and Kotlin
- `rust/crates/luster-dart` — flutter_rust_bridge bindings, which hand Dart the meshes
  as attribute arrays in flutter_scene's left-handed space
- `rust/crates/luster-cli` — `luster mint|artwork|export|parity|golden`
- `android/luster` — the Kotlin library: the bindings, `Luster.mint`, `LusterView`
  (Filament) and `LusterSnapshot`
- `android/luster-compose` — the same view as a composable, a thin layer over the stage
  `android/luster` draws with
- `android/sample` — the Android demo
- `flutter/luster` — the Flutter package: `LusterView`, the bindings, and the badge's
  own shader (`shaders/`), which `hook/build.dart` compiles for the app's Flutter SDK
- `Examples/`, `Apps/`, `Tools/` — the iOS demo, the macOS app, LusterShot
- `Scripts/` — building the engine, the parity check, releasing

## Releasing

Nothing publishes by itself. A release is prepared here and finished by hand.

```sh
Scripts/release.sh 0.2.0 --dry-run   # set the version, build, check, see the diff
Scripts/release.sh 0.2.0             # the same, then commit and tag locally
```

It sets the version in the engine, the Flutter package and the Android library, builds
the engine for every platform, runs the tests and the parity check, packs the
XCFramework and points `Package.swift` at the zip the release will carry — by URL and
checksum, so a consumer never needs Rust. It pushes nothing.

After the tag it commits `Package.swift` back to the local XCFramework, so work on the
branch goes on against the engine built from it.

When the release is really wanted, from the repository's root:

```sh
git push origin HEAD 0.2.0
gh release create 0.2.0 rust/target/apple/LusterFFI.xcframework.zip \
  --draft --verify-tag --title "Luster 0.2.0" --generate-notes     # the zip the tag names
gh workflow run release.yml -f version=0.2.0 -f publish=false   # check
gh workflow run release.yml -f version=0.2.0 -f publish=true    # publish
cd flutter/luster && dart pub publish     # pub.dev, by hand, once the workflow is done
```

The zip has to be the one `release.sh` packed: the engine never builds to the same
bytes twice (the XCFramework's slices come out in any order and its archives carry the
time they were made), so the workflow does not rebuild it. It refuses a tag whose
`Package.swift` still points at the local XCFramework, checks that the draft's zip has
the checksum the tag carries, runs the tests linked against that zip, and only then
takes the draft public. The Android libraries go to Maven Central as
`dev.famio:luster` and `dev.famio:luster-compose`, once the Apple side has passed,
signed with the key in the repository's secrets (`SIGNING_KEY`, `SIGNING_PASSWORD`;
the Central Portal token is `MAVEN_CENTRAL_USERNAME` and `MAVEN_CENTRAL_PASSWORD`).
A check run signs them too, without uploading.

A Flutter app needs no Rust either. With `publish`, the workflow also builds the
plugin's engine for every target it runs on, signs it with `CARGOKIT_PRIVATE_KEY` and
puts it in a `precompiled_<hash>` prerelease. cargokit, in an app's build, works out the
hash from the package's Rust sources, fetches those files and checks them against the
public key in `rust/crates/luster-dart/cargokit.yaml`. The hash covers luster-dart,
luster-core and the workspace's manifest and lock file (the vendored cargokit is changed
to take in all of them), so a change to the engine alone gets new files. That is why
`dart pub publish` waits for the workflow. `flutter/luster/rust` links to `rust/`, so
the package carries the sources, which the hash and a build from source both need.
`dart run build_tool gen-key` in `flutter/luster/cargokit/build_tool` makes a new key
pair.
