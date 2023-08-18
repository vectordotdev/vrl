#!/bin/bash

set -e

# Set environment variables
export RUST_BACKTRACE=full
export VECTOR_LOG=debug

SCRIPTS_DIR="$(dirname "$0")"
VALID_OPERATIONS=("clippy" "format_check" "tests" "vrl_tests" "check_features" "check_licenses" "check_wasm32")

function show_usage {
    echo "Usage: $0 <CHOSEN_OPERATION>"
    echo "Valid arguments: all ${VALID_OPERATIONS[*]}"
    exit 1
}

function check_exit_code {
    if [ $? -ne 0 ]; then
        echo "Error: $1 failed."
        exit 1
    fi
}

# Parse arguments; default to "all" if nothing is provided.
CHOSEN_OPERATION=${1:-"all"}

if [ "$CHOSEN_OPERATION" != "all" ]; then
  if [[ ! " ${VALID_OPERATIONS[*]} " =~ " ${CHOSEN_OPERATION} " ]]; then
      show_usage
  fi
fi

if [ "$CHOSEN_OPERATION" == "all" ]; then
    TO_BE_EXECUTED=("${VALID_OPERATIONS[@]}")
else
    TO_BE_EXECUTED=( "$CHOSEN_OPERATION" )
fi

for OPERATION in "${TO_BE_EXECUTED[@]}"; do
    "${SCRIPTS_DIR}"/"${OPERATION}".sh
    check_exit_code "${OPERATION}"
done

echo "Check(s) passed!"
