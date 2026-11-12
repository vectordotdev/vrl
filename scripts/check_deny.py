#!/usr/bin/env python3
"""Only fail on cargo-deny errors absent from a checked-out base revision.

Adapted from Vector's vdev/src/commands/check/deny.rs. Advisory IDs are stable
across affected crate versions; other errors retain their code, crates and message.
"""

import argparse
import json
from pathlib import Path
import subprocess
import sys


def deny_errors(directory):
    result = subprocess.run(
        ["cargo", "deny", "--log-level", "error", "--format", "json",
         "--all-features", "check", "all"],
        cwd=directory, capture_output=True, text=True, check=False,
    )
    errors = {}
    summary = None
    try:
        for line in result.stderr.splitlines():
            if not line.strip():
                continue
            entry = json.loads(line)
            fields = entry["fields"]
            if entry["type"] == "summary":
                summary = fields
            elif entry["type"] == "diagnostic" and fields["severity"] == "error":
                code = fields["code"]
                message = fields["message"]
                crates = [graph["Krate"] for graph in fields.get("graphs", [])]
                advisory = fields.get("advisory")
                if advisory:
                    key = ("advisory", advisory["id"])
                else:
                    # License diagnostics share a generic message. Include their
                    # labels, without source locations that move across revisions.
                    labels = tuple(sorted(
                        (label["message"], label["span"])
                        for label in fields.get("labels", [])
                    ))
                    key = (code, tuple(sorted(crate["name"] for crate in crates)), message, labels)
                description = ", ".join(
                    f'{crate["name"]} {crate["version"]}' for crate in crates
                )
                identifier = advisory["id"] if advisory else code
                errors[key] = f"{identifier}\t{description}: {message}"

        # A failing command alone cannot distinguish findings from a broken scan.
        # Require a completed summary and a status consistent with its diagnostics.
        if not isinstance(summary, dict) or not summary or any(
            not isinstance(stats, dict)
            or not isinstance(stats["errors"], int) or stats["errors"] < 0
            for stats in summary.values()
        ):
            raise ValueError("missing or invalid summary")
        has_errors = any(stats["errors"] > 0 for stats in summary.values())
        # cargo-deny uses different positive exit codes for different check types.
        if (has_errors != bool(errors) or result.returncode < 0
                or (result.returncode != 0) != has_errors):
            raise ValueError("exit status, summary and diagnostics disagree")
    except (ValueError, KeyError, TypeError) as error:
        raise RuntimeError(
            f"cargo deny did not complete reliably in {directory}: {error}\n"
            f"{result.stdout}{result.stderr}"
        ) from error
    return errors


def check(base_dir):
    # Scan head first, then base, as Vector does. An advisory published between
    # scans can appear only in the base, so it cannot spuriously fail the PR.
    head = deny_errors(Path.cwd())
    if not head:
        print("No cargo-deny errors found.")
        return 0
    base = deny_errors(base_dir)
    new = []
    for key, description in head.items():
        if key in base:
            print(f"Existing error (ignored): {description}")
        else:
            print(f"New error: {description}")
            new.append(key)
    if new:
        print("This change adds cargo-deny errors. Run `make check-deny` for details.")
        return 1
    print("No new cargo-deny errors compared to the base.")
    return 0


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--base-dir", required=True, type=Path)
    args = parser.parse_args()
    base = args.base_dir.resolve()
    if base == Path.cwd().resolve() or not (base / "Cargo.toml").is_file():
        parser.error("--base-dir must be a separate checkout containing Cargo.toml")
    try:
        return check(base)
    except (OSError, RuntimeError) as error:
        print(error, file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
