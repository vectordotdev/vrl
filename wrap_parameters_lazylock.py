#!/usr/bin/env python3
"""
Script to wrap parameters() methods that have defaults in LazyLock<Vec<Parameter>>.
"""

import re
from pathlib import Path
import sys

def wrap_parameters_in_lazylock(file_path: Path) -> bool:
    """Wrap parameters with defaults in LazyLock pattern."""
    with open(file_path, 'r') as f:
        content = f.read()

    # Check if this file has defaults
    if 'default: Some(&DEFAULT' not in content:
        return False

    # Check if already wrapped
    if 'static PARAMETERS: LazyLock<Vec<Parameter>>' in content:
        print(f"  Already wrapped: {file_path.name}")
        return False

    # Find the parameters() method
    pattern = r'fn parameters\(&self\) -> &\'static \[Parameter\] \{\s*&\[(.*?)\]\s*\}'
    match = re.search(pattern, content, re.DOTALL)

    if not match:
        print(f"  No parameters method found: {file_path.name}")
        return False

    params_content = match.group(1).strip()
    full_match = match.group(0)

    # Create the LazyLock static
    lazylock_static = f"""static PARAMETERS: LazyLock<Vec<Parameter>> = LazyLock::new(|| {{
    vec![
{params_content}
    ]
}});"""

    # Find where to insert the static (after other statics, before fn/struct/impl)
    lines = content.split('\n')
    insert_idx = 0
    last_static_idx = 0

    for i, line in enumerate(lines):
        stripped = line.strip()
        if stripped.startswith('static ') and not stripped.startswith('static PARAMETERS'):
            last_static_idx = i + 1
        elif stripped.startswith(('fn ', 'pub fn ', 'struct ', 'pub struct ', 'impl ', '#[derive')):
            if insert_idx == 0:
                insert_idx = max(last_static_idx, i)
            break

    if insert_idx == 0:
        insert_idx = last_static_idx

    # Insert the static
    lines.insert(insert_idx, '')
    lines.insert(insert_idx + 1, lazylock_static)
    lines.insert(insert_idx + 2, '')

    updated_content = '\n'.join(lines)

    # Replace the parameters() method
    new_method = """fn parameters(&self) -> &'static [Parameter] {
        PARAMETERS.as_slice()
    }"""

    updated_content = updated_content.replace(full_match, new_method)

    with open(file_path, 'w') as f:
        f.write(updated_content)

    print(f"  ✓ Wrapped: {file_path.name}")
    return True


def main():
    """Main entry point."""
    stdlib_dir = Path(__file__).parent / 'src' / 'stdlib'

    # Get list of files with defaults
    import subprocess
    result = subprocess.run(
        ['grep', '-r', 'default: Some(&DEFAULT', 'src/stdlib/', '--files-with-matches'],
        capture_output=True,
        text=True,
        cwd=Path(__file__).parent
    )

    files = [Path(f.strip()) for f in result.stdout.strip().split('\n') if f.strip()]
    print(f"Found {len(files)} files with defaults\n")

    modified = 0
    for file_path in sorted(files):
        if wrap_parameters_in_lazylock(file_path):
            modified += 1

    print(f"\nWrapped {modified} files")


if __name__ == '__main__':
    main()
