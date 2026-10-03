#!/bin/bash
# Prepares a release: sets the version everywhere, builds the engine, packs the
# XCFramework, points Package.swift at the zip a GitHub release will carry, and
# tags it.
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

if [ -n "$(git status --porcelain)" ]; then
    echo "the working tree has changes; commit or stash them first" >&2
    exit 1
fi
# The commit after the tag puts this manifest back, so it has to be the one
# that builds against the local engine.
if ! grep -q 'path: "rust/target/apple/LusterFFI.xcframework"' Package.swift; then
    echo "Package.swift does not point at the local XCFramework; release from a branch that does" >&2
    exit 1
fi

# The files the version is written into. A dry run puts back what it wrote,
# from copies rather than from git, so nothing else can be caught by it.
VERSIONED=(Package.swift rust/Cargo.toml flutter/luster/pubspec.yaml
           flutter/luster/android/build.gradle
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
/usr/bin/sed -i '' "s/^\( *versionName = \).*/\1\"$VERSION\"/" android/sample/build.gradle.kts
/usr/bin/sed -i '' "s/^\( *version = \).*/\1\"$VERSION\"/" android/luster/build.gradle.kts 2>/dev/null || true
/usr/bin/sed -i '' "s/^\( *version = \).*/\1\"$VERSION\"/" android/luster-compose/build.gradle.kts 2>/dev/null || true
/usr/bin/sed -i '' "s/^version '.*'/version '$VERSION'/" flutter/luster/android/build.gradle

say "engine"
Scripts/build-xcframework.sh release
Scripts/build-android.sh release

say "checks"
(cd rust && cargo test --workspace --quiet)
swift test
Scripts/parity.sh

say "packing the XCFramework"
rm -f "$ZIP"
(cd rust/target/apple && zip -q -r -X "$(basename "$ZIP")" LusterFFI.xcframework)
CHECKSUM="$(swift package compute-checksum "$ZIP")"
echo "$CHECKSUM  $ZIP"

say "pointing Package.swift at the release"
python3 - "$URL" "$CHECKSUM" <<'PY'
import re, sys
url, checksum = sys.argv[1], sys.argv[2]
p = 'Package.swift'
s = open(p).read()
target = '''let ffi: Target = .binaryTarget(
    name: "LusterFFI",
    url: "%s",
    checksum: "%s"
)''' % (url, checksum)
s = re.sub(r'let ffi: Target = \.binaryTarget\((?:[^)]*)\)', target, s, count=1)
open(p, 'w').write(s)
PY
# Not `swift build`: the zip is only reachable once the release carries it, so
# what can be checked here is that the manifest names it and its checksum.
grep -q "url: \"$URL\"" Package.swift && grep -q "checksum: \"$CHECKSUM\"" Package.swift
echo "Package.swift names the release's zip and its checksum"

if [ "$DRY" = 1 ]; then
    say "dry run"
    git --no-pager diff --stat
    echo
    echo "Putting the tree back as it was; nothing was committed, pushed or published."
    for file in "${VERSIONED[@]}"; do
        cp "$KEEP/$file" "$file"
    done
    rm -rf "$KEEP"
    exit 0
fi

say "committing and tagging"
git add -A Package.swift rust/Cargo.toml rust/Cargo.lock flutter/luster/pubspec.yaml \
    flutter/luster/android/build.gradle \
    android/luster/build.gradle.kts android/luster-compose/build.gradle.kts \
    android/sample/build.gradle.kts \
    Sources/LusterCore/Generated android/luster/src/main/kotlin/dev/famio/luster/core
git commit -m "Release $VERSION"
git tag -a "$VERSION" -m "Luster $VERSION"

# The tag names the release's zip; work after it goes on against the engine
# built here, or a change to the Rust would never reach the Swift that uses it.
say "pointing Package.swift back at the local engine"
git show "HEAD~1:Package.swift" > Package.swift
git commit -q -m "Build against the local engine again after $VERSION" Package.swift

cat <<EOF

Nothing has been pushed or published. When the release is really wanted, from
the repository's root:
  git push origin HEAD "$VERSION"
  gh release create "$VERSION" "$ZIP" --draft --verify-tag \\
    --title "Luster $VERSION" --generate-notes
  gh workflow run release.yml -f version=$VERSION -f publish=true

The zip has to be this one: the engine never builds to the same bytes twice,
and Package.swift names this one's checksum. Run the workflow with
publish=false first to check everything without releasing anything; with
publish=true it takes the draft public. pub.dev is always the last step by hand:
  cd flutter/luster && dart pub publish
EOF
