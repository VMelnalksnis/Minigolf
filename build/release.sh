#!/bin/bash
set -e

echo "Building WASM game client"
cargo install wasm-bindgen-cli
cargo build \
  --bin minigolf_client \
  --target wasm32-unknown-unknown \
  --profile wasm-release

wasm-bindgen \
  --no-typescript \
  --target web \
  --out-dir ./target/wasm32-unknown-unknown/wasm-release/ \
  --out-name "minigolf_client" \
  ./target/wasm32-unknown-unknown/wasm-release/minigolf_client.wasm

if [ "$1" == "native" ]; then
  echo "Building for the current CPU"
  export RUSTFLAGS="-C target-cpu=native"
fi

echo "Building lobby server"
cargo build --bin minigolf_lobby --release

echo "Building game server"
cargo build --bin minigolf_server --release
