#!/bin/bash
if ! cargo install --list | grep -q "cargo-hack v0.5.29"; then
    cargo install cargo-hack --version 0.5.29 --force --locked
fi

echo "Check that all features can compile"
cargo hack check --feature-powerset --depth 1
