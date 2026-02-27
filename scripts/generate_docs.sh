#!/bin/bash
set -euo pipefail

cargo run -p vrl-docs --features vrl_mock -- --output docs/
