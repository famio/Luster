#!/bin/bash
# Builds the engine for Android (arm64-v8a and x86_64) into the library
# module's jniLibs, and regenerates the Kotlin bindings beside them.
# Usage: Scripts/build-android.sh [release|debug]   (default: release)
set -euo pipefail

PROFILE="${1:-release}"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RUST="$ROOT/rust"
MODULE="$ROOT/android/luster/src/main"
export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"

# The newest NDK installed with the SDK.
SDK="${ANDROID_HOME:-$HOME/Library/Android/sdk}"
if [ -z "${ANDROID_NDK_HOME:-}" ]; then
    ANDROID_NDK_HOME="$SDK/ndk/$(ls "$SDK/ndk" | sort -V | tail -1)"
fi
export ANDROID_NDK_HOME
echo "ndk: $ANDROID_NDK_HOME"

case "$PROFILE" in
    release) FLAGS=(--release) ;;
    debug) FLAGS=() ;;
    *) echo "usage: $0 [release|debug]" >&2; exit 2 ;;
esac

(cd "$RUST" && cargo ndk -t arm64-v8a -t x86_64 -o "$MODULE/jniLibs" build "${FLAGS[@]}" -p luster-ffi)

# The bindings are the same whatever the profile, and a release build strips
# the metadata they are read from, so they come from a plain host build.
case "$(uname -s)" in
    Darwin) HOST_LIB=libluster_ffi.dylib ;;
    *) HOST_LIB=libluster_ffi.so ;;
esac
(cd "$RUST" && cargo build -q -p luster-ffi \
    && cargo run -q -p uniffi-bindgen -- generate \
        --library "$RUST/target/debug/$HOST_LIB" \
        --language kotlin --no-format --out-dir "$MODULE/kotlin")

# The bindings are the wrappers' to call, not the app's: every top-level
# declaration is made internal to the module, as the Swift side's are by its
# internal import. (Kotlin's internal is public to the JVM, so JNA's
# reflection still reaches the structures and the library interface.)
# Unformatted, a top-level function starts its line with a space or follows
# the end of its doc comment; a member's is an override.
perl -pi -e 's/^(?:public )?((?:open |data |sealed |enum |abstract |inline |value |annotation |const |suspend )*)(class|interface|object|fun|val|var|typealias)\b/internal $1$2/;
             s/^((?:\s*\*\/)? )((?:suspend )?fun )/$1internal $2/' \
    "$MODULE/kotlin/dev/famio/luster/core/luster_ffi.kt"

echo "built $MODULE/jniLibs and the Kotlin bindings"
