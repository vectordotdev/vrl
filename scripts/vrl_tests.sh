#!/bin/bash
set -e

cargo run --package vrl-tests --bin vrl-tests

# # Run skipped tests/examples like so
# cargo run --package vrl-tests --bin vrl-tests -- --run-skipped

