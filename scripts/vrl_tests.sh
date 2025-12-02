#!/bin/bash
cargo run --package vrl-tests --bin vrl-tests

echo "Running mocked tests"
cargo run --package vrl-tests --bin vrl-tests --features=vrl_mock
