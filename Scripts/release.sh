#!/bin/bash
# Prepares a release: sets the version everywhere, builds the engine, packs the
# XCFrameworks, points the Swift package and the Flutter plugin's at the zips a
# GitHub release will carry, and tags it.
#
#   Scripts/release.sh 0.2.0            # prepare, commit and tag
#   Scripts/release.sh 0.2.0 --dry-run  # do everything but commit and tag
#
# It never pushes and never publishes: it leaves a commit and a tag here, and
# prints what to run when the release is really wanted. The release workflow is
# started by hand as well, and only publishes when asked.
set -euo pipefail

VERSION="${1:-}"
DRY=0
[ "${2:-}" = "--dry-run" ] && DRY=1
if ! [[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    echo "usage: $0 <major.minor.patch> [--dry-run]" >&2
    exit 2
fi

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"
REPO="${LUSTER_REPO:-famio/Luster}"
ZIP="rust/target/apple/LusterFFI.xcframework.zip"
URL="https://github.com/$REPO/releases/download/$VERSION/LusterFFI.xcframework.zip"
FLUTTER_MANIFEST="flutter/luster/darwin/luster/Package.swift"
FLUTTER_ZIP="rust/target/flutter/luster_dart.xcframework.zip"
FLUTTER_URL="https://github.com/$REPO/releases/download/$VERSION/luster_dart.xcframework.zip"

if [ -n "$(git status --porcelain)" ]; then
    echo "the working tree has changes; commit or stash them first" >&2
    exit 1
fi
# The commit after the tag puts these manifests back, so they have to be the
# ones that build against the local engine.
if ! grep -q 'path: "rust/target/apple/LusterFFI.xcframework"' Package.swift ||
   ! grep -q 'path: "luster_dart.xcframework"' "$FLUTTER_MANIFEST"; then
    echo "a Package.swift does not point at the local XCFramework; release from a branch that does" >&2
    exit 1
fi

# The files the version is written into, here or by cargo as it builds. A dry
# run puts back what it wrote, from copies rather than from git, so nothing
# else can be caught by it.
VERSIONED=(Package.swift "$FLUTTER_MANIFEST" rust/Cargo.toml rust/Cargo.lock flutter/luster/pubspec.yaml
           flutter/luster/example/pubspec.lock flutter/luster/android/build.gradle
           android/luster/build.gradle.kts android/luster-compose/build.gradle.kts
           android/sample/build.gradle.kts)
KEEP="$(mktemp -d)"
if [ "$DRY" = 1 ]; then
    for file in "${VERSIONED[@]}"; do
        mkdir -p "$KEEP/$(dirname "$file")"
        cp "$file" "$KEEP/$file"
    done
fi

say() { printf '\n== %s\n' "$1"; }

say "version $VERSION"
# The engine's version is the package's; the bindings report it.
/usr/bin/sed -i '' "s/^version = \".*\"/version = \"$VERSION\"/" rust/Cargo.toml
/usr/bin/sed -i '' "s/^version: .*/version: $VERSION/" flutter/luster/pubspec.yaml
# The example's lock names the version of the package it is built on, as pub
# would write it on its next run.
/usr/bin/sed -i '' "/^  luster:\$/,/^    version:/ s/^    version: \".*\"/    version: \"$VERSION\"/" \
    flutter/luster/example/pubspec.lock
/usr/bin/sed -i '' "s/^\( *versionName = \).*/\1\"$VERSION\"/" android/sample/build.gradle.kts
/usr/bin/sed -i '' "s/^\( *version = \).*/\1\"$VERSION\"/" android/luster/build.gradle.kts 2>/dev/null || true
/usr/bin/sed -i '' "s/^\( *version = \).*/\1\"$VERSION\"/" android/luster-compose/build.gradle.kts 2>/dev/null || true
/usr/bin/sed -i '' "s/^version '.*'/version '$VERSION'/" flutter/luster/android/build.gradle

say "engine"
Scripts/build-xcframework.sh release
Scripts/build-flutter-xcframework.sh release
Scripts/build-android.sh release

say "checks"
(cd rust && cargo test --workspace --quiet)
swift test
Scripts/parity.sh

say "packing the XCFrameworks"
rm -f "$ZIP" "$FLUTTER_ZIP"
(cd rust/target/apple && zip -q -r -X "$(basename "$ZIP")" LusterFFI.xcframework)
# -y keeps the macOS framework's links as links.
(cd flutter/luster/darwin/luster &&
    zip -q -r -X -y "$ROOT/$FLUTTER_ZIP" luster_dart.xcframework)
CHECKSUM="$(swift package compute-checksum "$ZIP")"
FLUTTER_CHECKSUM="$(swift package compute-checksum "$FLUTTER_ZIP")"
echo "$CHECKSUM  $ZIP"
echo "$FLUTTER_CHECKSUM  $FLUTTER_ZIP"

say "pointing the Package.swift files at the release"
# The target named by `let <variable>: Target = .binaryTarget(...)` in a
# manifest becomes the zip at a URL, by its checksum.
point() {
    python3 - "$@" <<'PY'
import re, sys
manifest, variable, name, url, checksum = sys.argv[1:]
s = open(manifest).read()
target = '''let %s: Target = .binaryTarget(
    name: "%s",
    url: "%s",
    checksum: "%s"
)''' % (variable, name, url, checksum)
s, n = re.subn(r'let %s: Target = \.binaryTarget\((?:[^)]*)\)' % variable, target, s, count=1)
assert n == 1, manifest
open(manifest, 'w').write(s)
PY
    # Not `swift build`: the zip is only reachable once the release carries
    # it, so what can be checked here is that the manifest names it and its
    # checksum.
    grep -q "url: \"$4\"" "$1" || exit 1
    grep -q "checksum: \"$5\"" "$1" || exit 1
    echo "$1 names the release's zip and its checksum"
}
point Package.swift ffi LusterFFI "$URL" "$CHECKSUM"
point "$FLUTTER_MANIFEST" engine luster_dart "$FLUTTER_URL" "$FLUTTER_CHECKSUM"

if [ "$DRY" = 1 ]; then
    say "dry run"
    git --no-pager diff --stat
    echo
    echo "Putting the tree back as it was; nothing was committed, pushed or published."
    for file in "${VERSIONED[@]}"; do
        cp "$KEEP/$file" "$file"
    done
    rm -rf "$KEEP"
    # cargo still takes its builds of the version put back for fresh, though
    # the libraries they made now hold this run's version; with the version's
    # source newer than them, the next build makes them again.
    touch rust/crates/luster-core/src/lib.rs
    exit 0
fi

say "committing and tagging"
git add -A Package.swift "$FLUTTER_MANIFEST" rust/Cargo.toml rust/Cargo.lock flutter/luster/pubspec.yaml \
    flutter/luster/example/pubspec.lock flutter/luster/android/build.gradle \
    android/luster/build.gradle.kts android/luster-compose/build.gradle.kts \
    android/sample/build.gradle.kts \
    Sources/LusterCore/Generated android/luster/src/main/kotlin/dev/famio/luster/core
git commit -m "chore(release): $VERSION"
git tag -a "$VERSION" -m "Luster $VERSION"

# The tag names the release's zips; work after it goes on against the engine
# built here, or a change to the Rust would never reach the code that uses it.
say "pointing the Package.swift files back at the local engine"
git show "HEAD~1:Package.swift" > Package.swift
git show "HEAD~1:$FLUTTER_MANIFEST" > "$FLUTTER_MANIFEST"
git commit -q -m "chore: build against the local engine again after $VERSION" \
    Package.swift "$FLUTTER_MANIFEST"

cat <<EOF

Nothing has been pushed or published. When the release is really wanted, from
the repository's root:
  git push origin HEAD "$VERSION"
  gh release create "$VERSION" "$ZIP" "$FLUTTER_ZIP" --draft --verify-tag \\
    --title "Luster $VERSION" --generate-notes
  gh workflow run release.yml -f version=$VERSION -f publish=true

The zips have to be these: the engine never builds to the same bytes twice,
and the Package.swift files name these ones' checksums. Run the workflow with
publish=false first to check everything without releasing anything; with
publish=true it takes the draft public. pub.dev and the macOS app come last,
by hand. pub.dev from the tag, whose Package.swift names the release's zip (on
the branch it names an engine the package does not carry; nothing stops that):
  git checkout "$VERSION" && (cd flutter/luster && dart pub publish); git checkout -
  Apps/LusterMac/notarize.sh
  gh release upload "$VERSION" "Apps/LusterMac/.build/Luster-$VERSION-macos.zip"
EOF
