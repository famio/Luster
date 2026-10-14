#!/bin/bash
# Builds the engine for the web: luster-wasm for wasm32-unknown-unknown, and
# the JavaScript wasm-bindgen writes for it, into web/luster/engine, where the
# npm package takes them from.
#
#   Scripts/build-wasm.sh          # release
#   Scripts/build-wasm.sh debug
#
# wasm-bindgen's CLI has to be the version luster-wasm pins, which this reads
# from its Cargo.toml:
#
#   cargo install wasm-bindgen-cli --version <that version> --locked
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"
PROFILE="${1:-release}"

WANTED="$(sed -nE 's/^wasm-bindgen = "=([0-9.]+)"/\1/p' "$ROOT/rust/crates/luster-wasm/Cargo.toml")"
FOUND="$(wasm-bindgen --version 2> /dev/null | cut -d' ' -f2 || true)"
if [ "$FOUND" != "$WANTED" ]; then
    echo "wasm-bindgen ${FOUND:-is not installed}; luster-wasm wants $WANTED:" >&2
    echo "  cargo install wasm-bindgen-cli --version $WANTED --locked" >&2
    exit 1
fi

cd "$ROOT/rust"
# Not in rust-toolchain.toml, which everyone building the engine reads.
rustup target add wasm32-unknown-unknown > /dev/null
case "$PROFILE" in
    release) cargo build --release --quiet -p luster-wasm --target wasm32-unknown-unknown ;;
    debug) cargo build --quiet -p luster-wasm --target wasm32-unknown-unknown ;;
    *)
        echo "usage: Scripts/build-wasm.sh [release|debug]" >&2
        exit 2
        ;;
esac

OUT="$ROOT/web/luster/engine"
rm -rf "$OUT"
wasm-bindgen --target web --out-dir "$OUT" "target/wasm32-unknown-unknown/$PROFILE/luster_wasm.wasm"
echo "engine: $OUT ($(wc -c < "$OUT/luster_wasm_bg.wasm" | tr -d ' ') bytes of wasm)"
