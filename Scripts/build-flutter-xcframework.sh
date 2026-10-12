#!/bin/bash
# Builds the Flutter plugin's engine for iOS, the iOS simulator and macOS into
# flutter/luster/darwin/luster/luster_dart.xcframework, which the plugin's
# Package.swift uses in place of the one a release carries.
# Usage: Scripts/build-flutter-xcframework.sh [release|debug] [all|macos]
#   (default: release all; macos leaves out iOS, for a check that builds only
#   the macOS app)
set -euo pipefail

PROFILE="${1:-release}"
PLATFORMS="${2:-all}"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RUST="$ROOT/rust"
# Its own target directory: the deployment targets differ from the Swift
# package's, and sharing one would rebuild everything each time either runs.
TARGET_DIR="$RUST/target/flutter"
OUT="$TARGET_DIR/apple"
PACKAGE="$ROOT/flutter/luster/darwin/luster"
NAME=luster_dart

export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"
# The plugin's minimums, those of Package.swift.
IOS_MIN=13.0
MACOS_MIN=10.15
export IPHONEOS_DEPLOYMENT_TARGET=$IOS_MIN
export MACOSX_DEPLOYMENT_TARGET=$MACOS_MIN

case "$PROFILE" in
    release) DIR=release; FLAGS=(--release) ;;
    debug)   DIR=debug;   FLAGS=() ;;
    *) echo "unknown profile: $PROFILE" >&2; exit 64 ;;
esac

case "$PLATFORMS" in
    all)   TARGETS=(aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios
                    aarch64-apple-darwin x86_64-apple-darwin) ;;
    macos) TARGETS=(aarch64-apple-darwin x86_64-apple-darwin) ;;
    *) echo "unknown platforms: $PLATFORMS" >&2; exit 64 ;;
esac
for target in "${TARGETS[@]}"; do
    (cd "$RUST" && cargo build -p luster_dart ${FLAGS[@]+"${FLAGS[@]}"} \
        --target "$target" --target-dir "$TARGET_DIR")
done
lib() { echo "$TARGET_DIR/$1/$DIR/lib$NAME.dylib"; }
VERSION="$(grep -m1 '^version = ' "$RUST/Cargo.toml" | cut -d'"' -f2)"

# A dynamic framework, not a static library: Flutter links the plugins'
# packages statically into the app, which would strip the functions Dart
# looks up by name, and nothing else calls.
plist() {
    local platform="$1" extra="$2"
    cat <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>CFBundleDevelopmentRegion</key>
	<string>en</string>
	<key>CFBundleExecutable</key>
	<string>$NAME</string>
	<key>CFBundleIdentifier</key>
	<string>dev.famio.luster.dart</string>
	<key>CFBundleInfoDictionaryVersion</key>
	<string>6.0</string>
	<key>CFBundleName</key>
	<string>$NAME</string>
	<key>CFBundlePackageType</key>
	<string>FMWK</string>
	<key>CFBundleShortVersionString</key>
	<string>$VERSION</string>
	<key>CFBundleSupportedPlatforms</key>
	<array>
		<string>$platform</string>
	</array>
	<key>CFBundleVersion</key>
	<string>$VERSION</string>
$extra</dict>
</plist>
EOF
}

# iOS frameworks are flat; macOS ones keep their files under Versions/A.
ios_framework() {
    local dir="$OUT/$1/$NAME.framework" platform="$2"
    shift 2
    mkdir -p "$dir"
    lipo -create "$@" -output "$dir/$NAME"
    install_name_tool -id "@rpath/$NAME.framework/$NAME" "$dir/$NAME"
    plist "$platform" "	<key>MinimumOSVersion</key>
	<string>$IOS_MIN</string>
" > "$dir/Info.plist"
}
macos_framework() {
    local dir="$OUT/macos/$NAME.framework"
    mkdir -p "$dir/Versions/A/Resources"
    lipo -create "$@" -output "$dir/Versions/A/$NAME"
    install_name_tool -id "@rpath/$NAME.framework/Versions/A/$NAME" "$dir/Versions/A/$NAME"
    plist MacOSX "	<key>LSMinimumSystemVersion</key>
	<string>$MACOS_MIN</string>
" > "$dir/Versions/A/Resources/Info.plist"
    ln -s A "$dir/Versions/Current"
    ln -s "Versions/Current/$NAME" "$dir/$NAME"
    ln -s Versions/Current/Resources "$dir/Resources"
}

rm -rf "$OUT"
FRAMEWORKS=()
if [ "$PLATFORMS" = all ]; then
    ios_framework ios iPhoneOS "$(lib aarch64-apple-ios)"
    ios_framework ios-simulator iPhoneSimulator "$(lib aarch64-apple-ios-sim)" "$(lib x86_64-apple-ios)"
    FRAMEWORKS+=(-framework "$OUT/ios/$NAME.framework" -framework "$OUT/ios-simulator/$NAME.framework")
fi
macos_framework "$(lib aarch64-apple-darwin)" "$(lib x86_64-apple-darwin)"
FRAMEWORKS+=(-framework "$OUT/macos/$NAME.framework")

rm -rf "$PACKAGE/$NAME.xcframework"
xcodebuild -create-xcframework "${FRAMEWORKS[@]}" \
    -output "$PACKAGE/$NAME.xcframework" >/dev/null

echo "built $PACKAGE/$NAME.xcframework ($PROFILE)"
for slice in "$PACKAGE/$NAME.xcframework"/*/; do
    printf '  %-28s %s\n' "$(basename "$slice")" "$(du -h -L "$slice/$NAME.framework/$NAME" | cut -f1)"
done
