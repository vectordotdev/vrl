#!/usr/bin/env python3
import argparse
import os
import subprocess
import sys
from inspect import getsourcefile
from os.path import abspath

import semver
import tomlkit

from utils.validate_version import assert_version_is_not_published

RELEASE_DIR = os.path.dirname(abspath(getsourcefile(lambda: 0)))
REPO_ROOT_DIR = os.path.dirname(RELEASE_DIR)
CHANGELOG_DIR = os.path.join(REPO_ROOT_DIR, "changelog.d")
SCRIPTS_DIR = os.path.join(REPO_ROOT_DIR, "scripts")
SCRIPT_FILENAME = os.path.basename(getsourcefile(lambda: 0))

def get_current_version():
    """Read the current version from Cargo.toml using tomlkit"""
    toml_path = os.path.join(REPO_ROOT_DIR, "Cargo.toml")

    try:
        with open(toml_path, "r") as file:
            doc = tomlkit.parse(file.read())

        if "package" in doc and "version" in doc["package"]:
            return doc["package"]["version"]
        else:
            print("Error: The `version` field is not present in Cargo.toml [package] section.")
            sys.exit(1)
    except FileNotFoundError:
        print(f"Error: Cargo.toml not found at {toml_path}")
        sys.exit(1)
    except Exception as e:
        print(f"Error: Failed to parse Cargo.toml: {e}")
        sys.exit(1)

def overwrite_version(version, dry_run=False):
    """
    Update version in Cargo.toml using tomlkit.
    Preserves formatting, comments, and structure.
    """
    toml_path = os.path.join(REPO_ROOT_DIR, "Cargo.toml")

    try:
        with open(toml_path, "r") as file:
            content = file.read()
            doc = tomlkit.parse(content)
    except FileNotFoundError:
        print(f"Error: Cargo.toml not found at {toml_path}")
        sys.exit(1)
    except Exception as e:
        print(f"Error: Failed to parse Cargo.toml: {e}")
        sys.exit(1)

    # Check if [package] section exists
    if "package" not in doc:
        print("Error: [package] section not found in Cargo.toml")
        sys.exit(1)

    # Check if version field exists
    if "version" not in doc["package"]:
        print("Error: version field not found in [package] section")
        sys.exit(1)

    current_version = doc["package"]["version"]

    if current_version == version:
        print(f"Already at version {version}.")
        sys.exit(1)

    commit_message = f"chore(deps): change version from {current_version} with {version}"
    print(commit_message)

    if dry_run:
        print("Dry-run mode: Skipping version file write and commit.")
        return

    # Update version in the document
    doc["package"]["version"] = version

    # Write back to file (preserves formatting and comments)
    with open(toml_path, "w") as file:
        file.write(tomlkit.dumps(doc))

    # Update VRL version in Cargo.lock
    subprocess.run(["cargo", "update", "-p", "vrl"], check=True, cwd=REPO_ROOT_DIR)

    subprocess.run(["git", "commit", "-a", "-m", commit_message], check=True, cwd=REPO_ROOT_DIR)


def resolve_version(version_arg):
    """
    Resolve version argument to actual version string.
    Supports:
    - Exact version (e.g., "1.2.3")
    - "major" - bump major version
    - "minor" - bump minor version
    - "patch" - bump patch version
    """
    bump_types = ["major", "minor", "patch"]

    if version_arg.lower() in bump_types:
        current_version_str = get_current_version()
        current_version = semver.VersionInfo.parse(current_version_str)

        if version_arg.lower() == "major":
            new_version = current_version.bump_major()
        elif version_arg.lower() == "minor":
            new_version = current_version.bump_minor()
        elif version_arg.lower() == "patch":
            new_version = current_version.bump_patch()

        new_version_str = str(new_version)
        print(f"Bumping {version_arg} version: {current_version_str} -> {new_version_str}")
        return new_version_str
    else:
        # Assume it's an exact version
        return version_arg

def validate_version(version):
    try:
        semver.VersionInfo.parse(version)
    except ValueError:
        print(f"Invalid version: {version}. Please provide a valid SemVer string.")
        exit(1)

    assert_version_is_not_published(version)

def generate_changelog(dry_run=False):
    print("Generating changelog...")
    if dry_run:
        print("Dry-run mode: Skipping changelog generation and commit.")
        return
    subprocess.run(["./generate_release_changelog.sh", "--no-prompt"], check=True, cwd=SCRIPTS_DIR)
    subprocess.run(["git", "commit", "-a", "-m", "chore(releasing): generate changelog"],
                   check=True,
                   cwd=REPO_ROOT_DIR)

def create_branch(branch_name, dry_run=False):
    print(f"Would create branch: {branch_name}")
    if dry_run:
        print("Dry-run mode: Skipping branch creation.")
        return
    subprocess.run(["git", "checkout", "-b", branch_name], check=True, cwd=REPO_ROOT_DIR)
    subprocess.run(["git", "push", "-u", "origin", branch_name],
                   check=True,
                   cwd=REPO_ROOT_DIR)

def create_pull_request(branch_name, new_version, issue_link=None, dry_run=False):
    title = f"chore(releasing): Prepare {new_version} release"
    body = f"Generated with {SCRIPT_FILENAME}"

    if issue_link:
        body += f"\n\nRelated issue: {issue_link}"

    print(f"Creating pull request with title: {title}")
    if dry_run:
        print("Dry-run mode: Skipping PR creation.")
    else:
        try:
            subprocess.run(
                ["gh", "pr", "create", "--title", title, "--body", body, "--head", branch_name,
                 "--base", "main", "--label", "no-changelog"], check=True, cwd=REPO_ROOT_DIR)
        except subprocess.CalledProcessError as e:
            print(f"Failed to create pull request: {e}")

def main():
    parser = argparse.ArgumentParser(description="Prepare a new release")
    parser.add_argument("version", help="The new version to release (e.g., '1.2.3', 'major', 'minor', or 'patch')")
    parser.add_argument("--issue", "-i", dest="issue_link",
                        help="GitHub issue link to include in the PR body (e.g., 'https://github.com/owner/repo/issues/123')")
    parser.add_argument("--dry-run", action="store_true",
                        help="Run the script without making any changes (read-only)")
    args = parser.parse_args()

    # Resolve version (could be exact version or bump type)
    new_version = resolve_version(args.version)
    dry_run = args.dry_run
    issue_link = args.issue_link

    if not dry_run:
        validate_version(new_version)

    branch_name = f"prepare-{new_version}-release"
    create_branch(branch_name, dry_run)
    overwrite_version(new_version, dry_run)
    generate_changelog(dry_run)

    if not dry_run:
        subprocess.run(["git", "push"], check=True, cwd=REPO_ROOT_DIR)

    create_pull_request(branch_name, new_version, issue_link, dry_run)

    if dry_run:
        print("\nDry-run completed. No changes were made.")

if __name__ == "__main__":
    main()
