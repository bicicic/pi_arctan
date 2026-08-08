#!/usr/bin/env bash
set -euo pipefail

project_dir="$(cd "$(dirname "$0")/.." && pwd)"
cd "$project_dir"

command -v rustup >/dev/null || {
  echo "rustup is required (https://rustup.rs)" >&2
  exit 1
}
command -v wasm-bindgen >/dev/null || {
  echo "wasm-bindgen-cli 0.2.127 is required" >&2
  exit 1
}

cargo build --release --lib --target wasm32-unknown-unknown
wasm-bindgen \
  --target web \
  --out-dir docs/pkg \
  --out-name pi_arctan \
  target/wasm32-unknown-unknown/release/pi_arctan.wasm
