#!/bin/bash
echo "Add wasm32-unknown-unknown target"
rustup target add wasm32-unknown-unknown

echo "Run check"
cargo check --target wasm32-unknown-unknown --no-default-features --features stdlib
