#!/bin/bash
cargo install cargo-hack --version 0.5.29 --force --locked

echo "Check that all features can compile"
cargo hack check --feature-powerset --depth 1
