#!/bin/bash
# Formats the Swift with swift-format, the toolchain's own, by .swift-format
# at the root, and the Flutter package's Dart with dart format, by the
# page_width in its analysis_options.yaml; with --lint it changes nothing and
# fails on what it would.
#
#   Scripts/format.sh                # rewrite the files, both languages
#   Scripts/format.sh --lint         # only report, as CI does
#   Scripts/format.sh --lint swift   # one language: swift or dart
#
# What a generator writes is left as it writes it: the engine's bindings
# (Scripts/build-xcframework.sh, flutter_rust_bridge_codegen) and the Flutter
# example's plugin registrant. cargokit is someone else's.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

LINT=0
if [ "${1:-}" = "--lint" ]; then
    LINT=1
    shift
fi
LANGUAGES="${1:-swift dart}"

# The tracked files matching a pattern, but for those grep -v's arguments name.
tracked() {
    local pattern="$1"
    shift
    git ls-files "$pattern" | grep -v "$@"
}

for language in $LANGUAGES; do
    case "$language" in
        swift)
            FILES=()
            while IFS= read -r file; do FILES+=("$file"); done < <(tracked '*.swift' \
                -e '^Sources/LusterCore/Generated/' -e '/GeneratedPluginRegistrant\.swift$')
            if [ "$LINT" = 1 ]; then
                swift format lint --strict "${FILES[@]}"
            else
                swift format format --in-place "${FILES[@]}"
            fi
            ;;
        dart)
            FILES=()
            while IFS= read -r file; do FILES+=("$file"); done < <(tracked 'flutter/*.dart' \
                -e '/cargokit/' -e '^flutter/luster/lib/src/rust/')
            # The SDK .fvmrc pins, where fvm is; CI puts that one on the path.
            DART=(dart)
            command -v fvm > /dev/null && DART=(fvm dart)
            if [ "$LINT" = 1 ]; then
                (cd flutter/luster &&
                    "${DART[@]}" format --output=none --set-exit-if-changed "${FILES[@]#flutter/luster/}")
            else
                (cd flutter/luster && "${DART[@]}" format "${FILES[@]#flutter/luster/}")
            fi
            ;;
        *) echo "usage: $0 [--lint] [swift|dart]" >&2; exit 64 ;;
    esac
done
