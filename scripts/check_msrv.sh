#!/bin/bash

if ! cargo install --list | grep -q "cargo-msrv v0.15.1"; then
    echo "Install the 3rd-party license tool"
    cargo install cargo-msrv --version 0.15.1 --force --locked
fi

echo "Check that the MSRV is up to date"
cargo msrv verify
