#!/usr/bin/env python3
"""
Script to add descriptions to Parameter structs in VRL stdlib files.
Fetches descriptions from ../vector/website/data/docs.json
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
    # Look for the identifier() method implementation
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

    # Pattern to match Parameter struct instantiation (multiline)
    # This matches: Parameter { keyword: "name", kind: ..., required: ..., }
    pattern = r'Parameter\s*\{[^}]+\}'

    for match in re.finditer(pattern, content, re.MULTILINE | re.DOTALL):
        param_text = match.group(0)

        # Extract the keyword from this parameter
        keyword_match = re.search(r'keyword:\s*"([^"]+)"', param_text)
        if keyword_match:
            keyword = keyword_match.group(1)
            parameters.append((keyword, param_text))

    return parameters


def get_parameter_description(docs: Dict, function_name: str, param_name: str) -> str:
    """Get the description for a parameter from docs.json."""
    try:
        func_data = docs['remap']['functions'].get(function_name)
        if not func_data:
            return "TODO"

        arguments = func_data.get('arguments', [])
        for arg in arguments:
            if arg.get('name') == param_name:
                return arg.get('description', 'TODO')

        return "TODO"
    except (KeyError, TypeError):
        return "TODO"


def update_parameter_with_description(param_text: str, description: str) -> str:
    """
    Update a Parameter struct text to include the description field.
    Handles the case where description already exists or needs to be added.
    """
    # Escape quotes in the description for Rust string literals
    escaped_description = description.replace('\\', '\\\\').replace('"', '\\"')

    # Check if description field already exists
    if re.search(r'description:\s*"', param_text):
        # Replace existing description
        updated = re.sub(
            r'description:\s*"[^"]*"',
            f'description: "{escaped_description}"',
            param_text
        )
        return updated
    else:
        # Add description field before the closing brace
        # Find the position before the last comma or closing brace
        # We want to add after the 'required' field

        # Pattern: find "required: bool," and add description after it
        updated = re.sub(
            r'(required:\s*(?:true|false)\s*,)',
            f'\\1\n            description: "{escaped_description}",',
            param_text
        )

        # If that didn't work (no comma after required), try without comma
        if updated == param_text:
            updated = re.sub(
                r'(required:\s*(?:true|false)\s*)(})',
                f'\\1,\n            description: "{escaped_description}",\n        \\2',
                param_text
            )

        return updated


def process_file(file_path: Path, docs: Dict) -> bool:
    """
    Process a single Rust file and update Parameter descriptions.
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

    # Update content with descriptions
    updated_content = content
    modified = False

    for param_name, param_text in parameters:
        description = get_parameter_description(docs, function_name, param_name)
        print(f"    {param_name}: {description}")

        updated_param = update_parameter_with_description(param_text, description)

        if updated_param != param_text:
            # Replace in content
            updated_content = updated_content.replace(param_text, updated_param)
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
