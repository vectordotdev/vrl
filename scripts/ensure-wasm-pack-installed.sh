#! /usr/bin/env bash

if [[ "$(wasm-pack --version)" != "wasm-pack 0.13.1" ]] ; then
    echo "wasm-pack version 0.13.1 is not installed"
    cargo install --force --locked --version 0.13.1 wasm-pack
else
    echo "wasm-pack version 0.13.1 is installed already"
fi

brew install llvm
export PATH="/opt/homebrew/opt/llvm/bin:$PATH"
