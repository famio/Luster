#!/bin/bash
# Builds rust/target/apple/LusterFFI.xcframework (static libraries for iOS,
# the iOS simulator and macOS) and regenerates the Swift bindings in
# Sources/LusterCore/Generated.
# Usage: Scripts/build-xcframework.sh [release|debug]   (default: release)
set -euo pipefail

PROFILE="${1:-release}"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RUST="$ROOT/rust"
OUT="$RUST/target/apple"
GENERATED="$ROOT/Sources/LusterCore/Generated"
LIB=libluster_ffi.a

export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"
export IPHONEOS_DEPLOYMENT_TARGET=18.0
export MACOSX_DEPLOYMENT_TARGET=15.0

case "$PROFILE" in
    release) DIR=release; FLAGS=(--release) ;;
    debug)   DIR=debug;   FLAGS=() ;;
    *) echo "unknown profile: $PROFILE" >&2; exit 64 ;;
esac

TARGETS=(aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios
         aarch64-apple-darwin x86_64-apple-darwin)
# From rust/, so rustup uses the toolchain rust/rust-toolchain.toml names.
for target in "${TARGETS[@]}"; do
    (cd "$RUST" && cargo build -p luster-ffi ${FLAGS[@]+"${FLAGS[@]}"} --target "$target")
done
lib() { echo "$RUST/target/$1/$DIR/$LIB"; }

rm -rf "$OUT"
mkdir -p "$OUT/ios-simulator" "$OUT/macos" "$OUT/bindings" "$OUT/headers/LusterFFI"

# One slice per platform: the simulator and macOS slices are fat.
lipo -create "$(lib aarch64-apple-ios-sim)" "$(lib x86_64-apple-ios)" \
     -output "$OUT/ios-simulator/$LIB"
lipo -create "$(lib aarch64-apple-darwin)" "$(lib x86_64-apple-darwin)" \
     -output "$OUT/macos/$LIB"

# Bindings come from the library's own metadata, so they always match it.
# Library mode reads the crate's uniffi.toml through cargo metadata, which
# must run inside the workspace.
(cd "$RUST" && cargo run -q -p uniffi-bindgen -- \
    generate --library "$RUST/target/aarch64-apple-darwin/$DIR/libluster_ffi.dylib" \
    --language swift --out-dir "$OUT/bindings")

# The headers sit in a subdirectory: a module.modulemap at the top of
# Headers collides with other static XCFrameworks in the same app.
cp "$OUT/bindings/LusterFFI.h" "$OUT/headers/LusterFFI/"
cp "$OUT/bindings/LusterFFI.modulemap" "$OUT/headers/LusterFFI/module.modulemap"

xcodebuild -create-xcframework \
    -library "$(lib aarch64-apple-ios)" -headers "$OUT/headers" \
    -library "$OUT/ios-simulator/$LIB" -headers "$OUT/headers" \
    -library "$OUT/macos/$LIB" -headers "$OUT/headers" \
    -output "$OUT/LusterFFI.xcframework" >/dev/null

mkdir -p "$GENERATED"
cp "$OUT/bindings/LusterCore.swift" "$GENERATED/LusterCore.swift"

echo "built $OUT/LusterFFI.xcframework ($PROFILE)"
for slice in "$OUT"/LusterFFI.xcframework/*/; do
    [ -d "$slice" ] || continue
    printf '  %-28s %s\n' "$(basename "$slice")" "$(du -h "$slice/$LIB" | cut -f1)"
done
