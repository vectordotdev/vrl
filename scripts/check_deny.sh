#!/bin/bash

if ! cargo install --list | grep -q "cargo-deny v0.17.1"; then
    echo "Install cargo-deny"
    cargo install cargo-deny --version 0.17.1 --force --locked
fi

echo "Check deny"
cargo deny --log-level error --all-features check all
