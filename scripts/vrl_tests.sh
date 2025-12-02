#!/bin/bash
set -e

cargo run --package vrl-tests --bin vrl-tests

echo "Running mocked tests"
cargo run --package vrl-tests --bin vrl-tests --features=vrl_mock
