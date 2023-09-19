#!/bin/bash

set -e

# Set environment variables
export RUST_BACKTRACE=full
export VECTOR_LOG=debug

SCRIPTS_DIR="$(dirname "$0")"
VALID_OPERATIONS=("clippy" "format_check" "tests" "vrl_tests" "check_features" "check_licenses" "check_wasm32")

function show_usage {
    echo "Usage:"
    echo "$0 [CHOSEN_OPERATION]"
    echo "Valid operations: ${VALID_OPERATIONS[*]}"
    echo "This script will run all checks if there are no arguments."
}

function check_exit_code {
    if [ $? -ne 0 ]; then
        echo "Error: $1 failed."
        exit 1
    fi
}

if [ "$1" == "help" ]; then
  show_usage
  exit 0
fi

# Parse arguments; default to running all checks.
if [ $# -eq 0 ]; then
  TO_BE_EXECUTED=("${VALID_OPERATIONS[@]}")
else
  TO_BE_EXECUTED=( "$@" )
fi

for OPERATION in "${TO_BE_EXECUTED[@]}"; do
    "${SCRIPTS_DIR}"/"${OPERATION}".sh
    check_exit_code "${OPERATION}"
    echo "${OPERATION} passed"
done

echo "Check(s) passed!"
