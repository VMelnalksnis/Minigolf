#!/bin/bash
set -e

export CARGO_INCREMENTAL=false
export CARGO_PROFILE_DEV_DEBUG=false
export CARGO_PROFILE_TEST_DEBUG=false

echo "Building WASM game client"
cargo build \
  --bin minigolf_client \
  --target wasm32-unknown-unknown \
  --locked

if [ "$1" == "native" ]; then
  echo "Building for the current CPU"
  export RUSTFLAGS="-C target-cpu=native"
fi

echo "Building lobby server"
cargo build --bin minigolf_lobby --locked

echo "Building game server"
cargo build --bin minigolf_server --locked
