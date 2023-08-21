#!/bin/bash
cargo clippy --workspace --all-targets --features "test" -- -D warnings
