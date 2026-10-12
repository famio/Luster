#!/bin/bash
# Formats the Swift with swift-format, the toolchain's own, by .swift-format
# at the root; with --lint it changes nothing and fails on what it would.
#
#   Scripts/format.sh          # rewrite the files
#   Scripts/format.sh --lint   # only report, as CI does
#
# What a generator writes is left as it writes it: the engine's bindings
# (Scripts/build-xcframework.sh) and the Flutter example's plugin registrant.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

FILES=()
while IFS= read -r file; do
    FILES+=("$file")
done < <(git ls-files '*.swift' |
    grep -v -e '^Sources/LusterCore/Generated/' -e '/GeneratedPluginRegistrant\.swift$')

case "${1:-}" in
    "") swift format format --in-place "${FILES[@]}" ;;
    --lint) swift format lint --strict "${FILES[@]}" ;;
    *) echo "usage: $0 [--lint]" >&2; exit 64 ;;
esac
