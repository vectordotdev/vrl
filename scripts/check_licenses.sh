#!/bin/bash

if ! cargo install --list | grep -q "dd-rust-license-tool v1.0.1"; then
    echo "Install the 3rd-party license tool"
    cargo install dd-rust-license-tool --version 1.0.1 --force --locked
fi

echo "Check that the 3rd-party license file is up to date"
dd-rust-license-tool check
