#!/usr/bin/env bash
# Build the board engine (wasm) and start the web app at http://127.0.0.1:3737
set -euo pipefail
cd "$(dirname "$0")/.."
PROFILE="${PROFILE:-release}"
cargo build -p rupok-wasm --target wasm32-unknown-unknown --profile "$PROFILE"
OUT_DIR="target/wasm32-unknown-unknown/$([ "$PROFILE" = dev ] && echo debug || echo "$PROFILE")"
wasm-bindgen --target web --out-dir apps/board/src/wasm --out-name rupok "$OUT_DIR/rupok_wasm.wasm"
cd apps/board
if command -v bun >/dev/null; then bun install && exec bunx vite; else npm install && exec npx vite; fi
