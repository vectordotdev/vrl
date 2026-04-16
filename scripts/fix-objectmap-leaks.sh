#!/usr/bin/env bash
#
# fix-objectmap-leaks.sh
#
# Replaces direct BTreeMap usage with ObjectMap wherever the BTreeMap
# represents a KeyString -> Value map (i.e., is used as an ObjectMap).
#
# Safe to re-run after rebasing. Idempotent — already-fixed sites won't match.
#
# Requirements: comby (https://comby.dev)
#
# Usage:
#   ./scripts/fix-objectmap-leaks.sh          # dry-run (default)
#   ./scripts/fix-objectmap-leaks.sh --apply  # apply changes in-place
#
set -euo pipefail

APPLY=""
if [[ "${1:-}" == "--apply" ]]; then
  APPLY="-in-place"
  echo "==> Applying changes in-place"
else
  echo "==> Dry run (pass --apply to modify files)"
fi

cd "$(git rev-parse --show-toplevel)"

run_comby() {
  local match="$1"
  local rewrite="$2"
  local extensions="${3:-.rs}"

  if [[ -n "$APPLY" ]]; then
    comby "$match" "$rewrite" "$extensions" -in-place -matcher .rs 2>/dev/null
  else
    comby "$match" "$rewrite" "$extensions" -diff -matcher .rs 2>/dev/null
  fi
}

echo ""
echo "--- Group A: Comby patterns (context-safe replacements) ---"
echo ""

echo "[1] Value::Object(BTreeMap::new())"
run_comby \
  'Value::Object(BTreeMap::new())' \
  'Value::Object(ObjectMap::new())'

echo "[2] Value::Object(BTreeMap::default())"
run_comby \
  'Value::Object(BTreeMap::default())' \
  'Value::Object(ObjectMap::default())'

echo "[3] Value::Object(std::collections::BTreeMap::new())"
run_comby \
  'Value::Object(std::collections::BTreeMap::new())' \
  'Value::Object(ObjectMap::new())'

echo "[4] Value::from(BTreeMap::new())"
run_comby \
  'Value::from(BTreeMap::new())' \
  'Value::from(ObjectMap::new())'

echo "[5] Value::from(BTreeMap::default())"
run_comby \
  'Value::from(BTreeMap::default())' \
  'Value::from(ObjectMap::default())'

echo "[6] Value::Object(BTreeMap::from(...))"
# Run twice: comby treats matched holes as opaque, so nested
# occurrences inside :[args] aren't rewritten on the first pass.
run_comby \
  'Value::Object(BTreeMap::from(:[args]))' \
  'Value::Object(ObjectMap::from(:[args]))'
run_comby \
  'Value::Object(BTreeMap::from(:[args]))' \
  'Value::Object(ObjectMap::from(:[args]))'

echo "[7] .collect() used as ObjectMap (in serde Value conversion)"
run_comby \
  '.map(|(key, value)| (key.into(), Self::from(value))).collect()' \
  '.map(|(key, value)| (key.into(), Self::from(value))).collect::<ObjectMap>()'

echo "[8] let mut x: Value = BTreeMap::new().into()"
run_comby \
  'let mut :[var~\w+]: Value = BTreeMap::new().into()' \
  'let mut :[var]: Value = ObjectMap::new().into()'

echo "[9] .collect::<BTreeMap<_, _>>() in Value context (cmd.rs)"
# comby doesn't handle turbofish well, so we target this file directly
group_a_collect_files=(
  "src/cli/cmd.rs"
)
for file in "${group_a_collect_files[@]}"; do
  if [[ -f "$file" ]]; then
    if [[ -n "$APPLY" ]]; then
      sed -i '' 's/collect::<BTreeMap<_, _>>()/collect::<ObjectMap>()/g' "$file"
    else
      grep -n 'collect::<BTreeMap<_, _>>()' "$file" 2>/dev/null | while read -r line; do
        echo "    would replace: $line"
      done
    fi
  fi
done

echo ""
echo "--- Group B: Separated constructions (targeted by file) ---"
echo ""

# These are BTreeMap::new() calls where the variable is used as ObjectMap
# but the construction and use are on different lines.
# We target specific files to avoid false positives on non-ObjectMap BTreeMaps.

group_b_files=(
  "src/parsing/xml.rs"
  "src/parsing/query_string.rs"
  "src/stdlib/parse_syslog.rs"
  "src/stdlib/parse_key_value.rs"
  "src/stdlib/parse_grok.rs"
  "src/stdlib/parse_aws_vpc_flow_log.rs"
  "src/stdlib/replace_with.rs"
  "src/value/value/serde.rs"
  "src/value/value/crud/insert.rs"
)

for file in "${group_b_files[@]}"; do
  if [[ -f "$file" ]]; then
    echo "  -> $file"
    if [[ -n "$APPLY" ]]; then
      sed -i '' 's/BTreeMap::new()/ObjectMap::new()/g' "$file"
    else
      grep -n 'BTreeMap::new()' "$file" 2>/dev/null | while read -r line; do
        echo "    would replace: $line"
      done
    fi
  fi
done

echo ""
echo "--- Group C: BTreeMap::from(...).into() in test files ---"
echo ""

# These are standalone BTreeMap::from([...]).into() calls that produce
# a Value::Object but aren't wrapped in Value::Object(...) directly.
group_c_files=(
  "src/value/value/iter.rs"
)

for file in "${group_c_files[@]}"; do
  if [[ -f "$file" ]]; then
    echo "  -> $file"
    if [[ -n "$APPLY" ]]; then
      sed -i '' 's/BTreeMap::from/ObjectMap::from/g' "$file"
    else
      grep -n 'BTreeMap::from' "$file" 2>/dev/null | while read -r line; do
        echo "    would replace: $line"
      done
    fi
  fi
done

echo ""
echo "--- Macros ---"
echo ""

echo "[value! macro] src/value/mod.rs"
if [[ -n "$APPLY" ]]; then
  sed -i '' 's|::std::collections::BTreeMap::default()|$crate::value::ObjectMap::default()|' \
    src/value/mod.rs
  sed -i '' 's|collect::<::std::collections::BTreeMap<_, $crate::value::Value>>()|collect::<$crate::value::ObjectMap>()|' \
    src/value/mod.rs
else
  grep -n 'BTreeMap' src/value/mod.rs | head -5
fi

echo "[btreemap! macro] src/value/btreemap.rs"
echo "  NOTE: btreemap! macro intentionally returns BTreeMap. Consider renaming"
echo "  to objectmap! or updating return type in a separate step."

echo ""
echo "--- Import fixups ---"
echo ""

# Files where BTreeMap import should be replaced with ObjectMap
swap_import_files=(
  "src/value/value/crud/insert.rs"
  "src/value/value/serde.rs"
  "src/parsing/query_string.rs"
  "src/datadog/grok/filters/keyvalue.rs"
  "src/stdlib/parse_grok.rs"
  "src/stdlib/encode_key_value.rs"
  "src/stdlib/from_unix_timestamp.rs"
  "src/stdlib/shannon_entropy.rs"
  "src/stdlib/uuid_v4.rs"
  "src/stdlib/uuid_v7.rs"
)

# Files where BTreeMap import should just be removed (ObjectMap comes via prelude or other import)
remove_import_files=(
  "src/cli/repl.rs"
  "lib/fuzz/src/main.rs"
)

# Files where ObjectMap import needs to be added (no prelude, no existing import)
add_objectmap_import=(
  "src/cli/cmd.rs:use crate::value::ObjectMap;"
  "src/cli/repl.rs:use crate::value::ObjectMap;"
  "examples/simple.rs"  # handled specially below
)

if [[ -n "$APPLY" ]]; then
  # Swap BTreeMap -> ObjectMap imports
  for file in "${swap_import_files[@]}"; do
    if [[ -f "$file" ]]; then
      # In test modules: use std::collections::BTreeMap -> use crate::value::ObjectMap
      sed -i '' 's/use std::collections::BTreeMap;/use crate::value::ObjectMap;/' "$file"
      # In main code: remove BTreeMap from grouped imports if it becomes unused
      # Handle "collections::BTreeMap" in grouped use statements
      sed -i '' 's/collections::BTreeMap, //' "$file"
      sed -i '' 's/, collections::BTreeMap//' "$file"
      sed -i '' 's/collections::BTreeMap//' "$file"
    fi
  done

  # Remove now-unused BTreeMap imports
  for file in "${remove_import_files[@]}"; do
    if [[ -f "$file" ]]; then
      sed -i '' '/^use std::collections::BTreeMap;$/d' "$file"
    fi
  done

  # For files in src/value/ that need ObjectMap but aren't in the compiler prelude
  # crud/insert.rs
  if ! grep -q 'use.*ObjectMap' src/value/value/crud/insert.rs 2>/dev/null; then
    sed -i '' '/^use crate::value::Value;$/a\
use crate::value::value::ObjectMap;
' src/value/value/crud/insert.rs
  fi

  # serde.rs
  if ! grep -q 'use.*ObjectMap' src/value/value/serde.rs 2>/dev/null; then
    sed -i '' '1s/^/use super::ObjectMap;\n/' src/value/value/serde.rs
  fi

  # keyvalue.rs
  if ! grep -q 'use.*ObjectMap' src/datadog/grok/filters/keyvalue.rs 2>/dev/null; then
    sed -i '' '/^use crate::value::Value;/s/Value}/Value, value::ObjectMap}/' src/datadog/grok/filters/keyvalue.rs 2>/dev/null || \
    sed -i '' '/^use crate::value::Value;$/a\
use crate::value::value::ObjectMap;
' src/datadog/grok/filters/keyvalue.rs
  fi

  # xml.rs: remove BTreeMap from grouped import, keep btree_map::Entry
  sed -i '' 's/collections::{BTreeMap, btree_map::Entry}/collections::btree_map::Entry/' src/parsing/xml.rs 2>/dev/null || true

  # parse_key_value.rs: remove BTreeMap from grouped import, keep btree_map::Entry
  sed -i '' 's/collections::{BTreeMap, btree_map::Entry}/collections::btree_map::Entry/' src/stdlib/parse_key_value.rs 2>/dev/null || true

  # cli/cmd.rs: add ObjectMap import
  if ! grep -q 'use crate::value::ObjectMap' src/cli/cmd.rs 2>/dev/null; then
    sed -i '' '/^use crate::compiler::TargetValueRef;$/a\
use crate::value::ObjectMap;
' src/cli/cmd.rs
  fi
  # Remove BTreeMap from grouped import in cmd.rs
  sed -i '' 's/    collections::BTreeMap,\n//' src/cli/cmd.rs 2>/dev/null || true
  sed -i '' '/^    collections::BTreeMap,$/d' src/cli/cmd.rs

  # cli/repl.rs: add ObjectMap import
  if ! grep -q 'use crate::value::ObjectMap' src/cli/repl.rs 2>/dev/null; then
    sed -i '' '/^use crate::value::Value;$/i\
use crate::value::ObjectMap;
' src/cli/repl.rs
  fi

  # examples/simple.rs: swap BTreeMap for ObjectMap in vrl import
  sed -i '' 's/use std::collections::BTreeMap;//' examples/simple.rs 2>/dev/null || true
  sed -i '' 's/value::{Secrets, Value}/value::{ObjectMap, Secrets, Value}/' examples/simple.rs 2>/dev/null || true

  # value/value/iter.rs: swap BTreeMap for ObjectMap in test module
  sed -i '' 's/use std::collections::{BTreeMap, HashMap};/use std::collections::HashMap;/' src/value/value/iter.rs 2>/dev/null || true
  if ! grep -q 'use crate::value::value::ObjectMap' src/value/value/iter.rs 2>/dev/null; then
    sed -i '' '/use std::collections::HashMap;/a\
    use crate::value::value::ObjectMap;
' src/value/value/iter.rs
  fi

  # value/value/path.rs: swap BTreeMap for ObjectMap in test module
  sed -i '' 's/use std::collections::BTreeMap;/use crate::value::value::ObjectMap;/' src/value/value/path.rs 2>/dev/null || true

  # datadog/grok/parse_grok.rs: the top-level BTreeMap import is no longer needed
  # (ObjectMap is already imported), but the test module needs BTreeMap for
  # parse_grok_rules() which takes BTreeMap<KeyString, String> (aliases, not ObjectMap).
  sed -i '' '/^use std::collections::BTreeMap;$/d' src/datadog/grok/parse_grok.rs 2>/dev/null || true
  # Add BTreeMap import to the test module if not already present
  if grep -q '#\[cfg(test)\]' src/datadog/grok/parse_grok.rs 2>/dev/null; then
    if ! grep -q 'use std::collections::BTreeMap' src/datadog/grok/parse_grok.rs 2>/dev/null; then
      sed -i '' '/^mod tests {$/a\
    use std::collections::BTreeMap;
' src/datadog/grok/parse_grok.rs
    fi
  fi
else
  echo "  (import fixups shown only in --apply mode)"
fi

echo ""
echo "--- Formatting & validation ---"
echo ""

if [[ -n "$APPLY" ]]; then
  echo "Running cargo fmt..."
  cargo fmt

  echo "Running cargo check..."
  if cargo check 2>&1 | tail -3; then
    echo ""
    echo "Running cargo test..."
    if cargo test 2>&1 | tail -3; then
      echo ""
      echo "==> All changes applied and validated successfully."
    else
      echo ""
      echo "==> Tests failed. Review errors above."
    fi
  else
    echo ""
    echo "==> cargo check found issues. Review errors above."
  fi
else
  echo "==> Dry run complete. Run with --apply to modify files."
fi
