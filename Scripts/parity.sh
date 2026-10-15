#!/bin/bash
# Checks the engine against the dumps in fixtures/golden: what it reads from a
# document, the cells it makes of it, the pieces it strikes, with its lines
# plated and left in the metal, and a hash of each GLB it writes, with no
# version in it. The dumps are this engine's own record: anything that moves in
# one of them is a difference, down to a single byte of a badge.
#
#   Scripts/parity.sh        # the engine built for this machine
#   Scripts/parity.sh wasm   # built for WebAssembly, run by Node's WASI
#
# `luster golden fixtures/golden fixtures/svg/*.svg` rewrites the committed
# dumps, for a change that is meant to move them; `luster golden <dir> <svg>…`
# writes one for a document added later.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"

case "${1:-}" in
    "")
        (cd "$ROOT/rust" && cargo build --release --quiet -p luster-cli)
        LUSTER=("$ROOT/rust/target/release/luster")
        ;;
    wasm)
        # Not in rust-toolchain.toml, which everyone building the engine reads.
        (cd "$ROOT/rust" && rustup target add wasm32-wasip1 > /dev/null &&
            cargo build --release --quiet -p luster-cli --target wasm32-wasip1)
        LUSTER=(node --no-warnings "$ROOT/Scripts/wasi.mjs" "$ROOT/rust/target/wasm32-wasip1/release/luster.wasm")
        ;;
    *)
        echo "usage: Scripts/parity.sh [wasm]" >&2
        exit 2
        ;;
esac

"${LUSTER[@]}" parity "$ROOT/fixtures/golden" "$ROOT/fixtures/svg"/*.svg
echo "svg${1:+ ($1)}: matches fixtures/golden"
