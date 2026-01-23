update_parameter_defaults_lazylock.py#!/usr/bin/env python3
"""
Script to add default values using LazyLock to Parameter structs in VRL stdlib files.
Fetches default values from ../vector/website/data/docs.json
"""

import json
import re
import os
from pathlib import Path
from typing import Dict, List, Optional, Tuple


def load_docs_json(path: str) -> Dict:
    """Load and parse the docs.json file."""
    with open(path, 'r') as f:
        return json.load(f)


def extract_function_identifier(content: str) -> Optional[str]:
    """Extract the function identifier from a Rust file."""
    pattern = r'fn identifier\(&self\) -> &\'static str \{\s*"([^"]+)"'
    match = re.search(pattern, content)
    if match:
        return match.group(1)
    return None


def extract_parameters(content: str) -> List[Tuple[str, str]]:
    """
    Extract Parameter instances from the content.
    Returns a list of (parameter_keyword, full_parameter_text) tuples.
    """
    parameters = []
    pattern = r'Parameter\s*\{[^}]+\}'

    for match in re.finditer(pattern, content, re.MULTILINE | re.DOTALL):
        param_text = match.group(0)
        keyword_match = re.search(r'keyword:\s*"([^"]+)"', param_text)
        if keyword_match:
            keyword = keyword_match.group(1)
            parameters.append((keyword, param_text))

    return parameters


def get_parameter_default_info(docs: Dict, function_name: str, param_name: str) -> Optional[Tuple[str, str]]:
    """Get the default value for a parameter from docs.json.

    Returns:
        Tuple of (static_name, value_constructor) or None if no default exists
    """
    try:
        func_data = docs['remap']['functions'].get(function_name)
        if not func_data:
            return None

        arguments = func_data.get('arguments', [])
        for arg in arguments:
            if arg.get('name') == param_name:
                default = arg.get('default')
                if default is not None:
                    # Generate static name (e.g., DEFAULT_CASE_SENSITIVE)
                    static_name = f"DEFAULT_{param_name.upper()}"

                    # Convert Python types to Rust Value constructors
                    if isinstance(default, bool):
                        value_constructor = "Value::Boolean(true)" if default else "Value::Boolean(false)"
                    elif isinstance(default, str):
                        # Escape the string for Rust
                        escaped = default.replace('\\', '\\\\').replace('"', '\\"')
                        value_constructor = f'Value::Bytes(Bytes::from("{escaped}"))'
                    elif isinstance(default, int):
                        value_constructor = f'Value::Integer({default})'
                    elif isinstance(default, float):
                        raise ValueError(f"Float default values are not supported: {function_name}.{param_name} = {default}")
                    else:
                        raise ValueError(f"Unsupported default value type: {type(default)} for {function_name}.{param_name}")

                    return (static_name, value_constructor)
                return None

        return None
    except (KeyError, TypeError):
        return None


def add_imports_if_needed(content: str, needs_bytes: bool, needs_lazylock: bool) -> str:
    """Add necessary imports if they don't exist."""
    lines = content.split('\n')

    # Find where to insert imports (after existing use statements)
    insert_idx = 0
    for i, line in enumerate(lines):
        if line.startswith('use ') or line.startswith('pub use '):
            insert_idx = i + 1
        elif line.startswith('fn ') or line.startswith('pub fn ') or line.startswith('struct ') or line.startswith('pub struct '):
            break

    # Check if imports already exist
    has_bytes = any('use bytes::Bytes' in line or 'use crate::compiler::prelude::*' in line for line in lines)
    has_lazylock = any('use std::sync::LazyLock' in line for line in lines)

    imports_to_add = []
    if needs_lazylock and not has_lazylock:
        imports_to_add.append('use std::sync::LazyLock;')
    if needs_bytes and not has_bytes:
        imports_to_add.append('use bytes::Bytes;')

    if imports_to_add:
        # Insert after existing imports
        for imp in reversed(imports_to_add):
            lines.insert(insert_idx, imp)

    return '\n'.join(lines)


def add_static_declarations(content: str, statics: List[Tuple[str, str]]) -> str:
    """Add LazyLock static declarations after imports."""
    if not statics:
        return content

    lines = content.split('\n')

    # Find where to insert statics (after imports, before first fn/struct/impl)
    insert_idx = 0
    for i, line in enumerate(lines):
        stripped = line.strip()
        if stripped.startswith('fn ') or stripped.startswith('pub fn ') or \
           stripped.startswith('struct ') or stripped.startswith('pub struct ') or \
           stripped.startswith('impl ') or stripped.startswith('const ') or \
           stripped.startswith('#[derive'):
            insert_idx = i
            break
        elif stripped and not stripped.startswith('use ') and not stripped.startswith('//'):
            insert_idx = i

    # Generate static declarations
    static_lines = []
    for static_name, value_constructor in statics:
        static_lines.append(f'static {static_name}: LazyLock<Value> = LazyLock::new(|| {value_constructor});')

    # Insert with blank line before
    if insert_idx > 0:
        static_lines.insert(0, '')
    static_lines.append('')

    for line in reversed(static_lines):
        lines.insert(insert_idx, line)

    return '\n'.join(lines)


def update_parameter_with_default_ref(param_text: str, static_name: Optional[str]) -> str:
    """Update a Parameter struct to reference a static default value."""
    # Check if default field already exists
    if re.search(r'default:\s*', param_text):
        # Replace existing default
        if static_name is not None:
            updated = re.sub(
                r'default:\s*(?:Some\([^)]+\)|None)',
                f'default: Some(&{static_name})',
                param_text
            )
        else:
            updated = re.sub(
                r'default:\s*(?:Some\([^)]+\)|None)',
                'default: None',
                param_text
            )
        return updated
    else:
        # Add default field after the description field
        if static_name is not None:
            updated = re.sub(
                r'(description:\s*"[^"]*",)',
                f'\\1\n            default: Some(&{static_name}),',
                param_text
            )
        else:
            updated = re.sub(
                r'(description:\s*"[^"]*",)',
                f'\\1\n            default: None,',
                param_text
            )
        return updated


def process_file(file_path: Path, docs: Dict) -> bool:
    """
    Process a single Rust file and update Parameter default values using LazyLock.
    Returns True if file was modified, False otherwise.
    """
    print(f"Processing {file_path.name}...")

    with open(file_path, 'r') as f:
        content = f.read()

    # Extract function identifier
    function_name = extract_function_identifier(content)
    if not function_name:
        print(f"  No function identifier found, skipping")
        return False

    print(f"  Function: {function_name}")

    # Extract parameters
    parameters = extract_parameters(content)
    if not parameters:
        print(f"  No parameters found")
        return False

    print(f"  Found {len(parameters)} parameter(s)")

    # Collect all defaults we need to create statics for
    statics_to_add = []
    param_updates = []
    needs_bytes = False
    needs_lazylock = False

    for param_name, param_text in parameters:
        default_info = get_parameter_default_info(docs, function_name, param_name)
        if default_info is not None:
            static_name, value_constructor = default_info
            statics_to_add.append((static_name, value_constructor))
            param_updates.append((param_text, static_name))
            needs_lazylock = True
            if 'Bytes::from' in value_constructor:
                needs_bytes = True
            print(f"    {param_name}: default = {static_name}")
        else:
            param_updates.append((param_text, None))
            print(f"    {param_name}: no default")

    # Update content
    updated_content = content
    modified = False

    # Update parameter definitions
    for param_text, static_name in param_updates:
        updated_param = update_parameter_with_default_ref(param_text, static_name)
        if updated_param != param_text:
            updated_content = updated_content.replace(param_text, updated_param)
            modified = True

    # Add static declarations
    if statics_to_add:
        updated_content = add_static_declarations(updated_content, statics_to_add)
        modified = True

    # Add imports if needed
    if needs_bytes or needs_lazylock:
        updated_content = add_imports_if_needed(updated_content, needs_bytes, needs_lazylock)
        modified = True

    if modified:
        with open(file_path, 'w') as f:
            f.write(updated_content)
        print(f"  ✓ Updated {file_path.name}")
    else:
        print(f"  No changes needed")

    return modified


def main():
    """Main entry point."""
    # Paths
    script_dir = Path(__file__).parent
    stdlib_dir = script_dir / 'src' / 'stdlib'
    docs_path = script_dir.parent / 'vector' / 'website' / 'data' / 'docs.json'

    # Load docs.json
    print(f"Loading docs from {docs_path}")
    docs = load_docs_json(docs_path)
    print(f"Loaded {len(docs.get('remap', {}).get('functions', {}))} functions from docs.json\n")

    # Find all .rs files in stdlib
    rs_files = list(stdlib_dir.rglob('*.rs'))
    print(f"Found {len(rs_files)} Rust files in {stdlib_dir}\n")

    # Process each file
    modified_count = 0
    for rs_file in sorted(rs_files):
        if process_file(rs_file, docs):
            modified_count += 1
        print()

    print(f"\nSummary: Modified {modified_count} out of {len(rs_files)} files")


if __name__ == '__main__':
    main()
