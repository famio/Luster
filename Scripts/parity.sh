#!/bin/bash
# Checks the engine against the dumps in fixtures/golden: what it reads from a
# document, the cells it makes of it, the pieces it strikes, with its lines
# plated and left in the metal, and a hash of each GLB it writes. The dumps are
# this engine's own record: anything that moves in one of them is a difference,
# down to a single byte of a badge.
#
#   Scripts/parity.sh
#
# `luster golden fixtures/golden fixtures/svg/*.svg` rewrites the committed
# dumps, for a change that is meant to move them; `luster golden <dir> <svg>…`
# writes one for a document added later.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"

(cd "$ROOT/rust" && cargo build --release --quiet -p luster-cli)
LUSTER="$ROOT/rust/target/release/luster"

"$LUSTER" parity "$ROOT/fixtures/golden" "$ROOT/fixtures/svg"/*.svg
echo "svg: matches fixtures/golden"
