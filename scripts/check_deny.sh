#!/bin/bash

if ! cargo install --list | grep -q "cargo-deny v0.18.9"; then
    echo "Install cargo-deny"
    cargo install cargo-deny --version 0.18.9 --force --locked
fi

echo "Check deny"
cargo deny --log-level error --all-features check all
