import argparse
import os
import subprocess
from inspect import getsourcefile
from os.path import abspath

import semver

SCRIPTS_DIR = os.path.dirname(abspath(getsourcefile(lambda: 0)))
REPO_ROOT_DIR = os.path.dirname(SCRIPTS_DIR)
CHANGELOG_DIR = os.path.join(REPO_ROOT_DIR, "changelog.d")


def overwrite_version(version):
    toml_path = os.path.join(REPO_ROOT_DIR, "Cargo.toml")
    with open(toml_path, "r") as file:
        lines = file.readlines()

    # This will preserve line order.
    current_version = None
    for i, line in enumerate(lines):
        if line.startswith("version ="):
            current_version = line.split("=")[1].strip().strip('"')
            if current_version == version:
                print(f"Already at version {version}.")
                exit(1)
            lines[i] = f"version = \"{version}\"\n"
            break

    if current_version is None:
        print("Version field not found in Cargo.toml.")
        exit(1)

    commit_message = f"chore(deps): change version from {current_version} with {version}"
    print(commit_message)

    with open(toml_path, "w") as file:
        file.writelines(lines)
    subprocess.run(["git", "commit", "-am", commit_message], check=True, cwd=REPO_ROOT_DIR)


def validate_version(version):
    try:
        semver.VersionInfo.parse(version)
    except ValueError:
        print(f"Invalid version: {version}. Please provide a valid SemVer string.")
        exit(1)


def generate_changelog():
    print("Generating changelog...")
    subprocess.run(["generate_release_changelog.sh"], check=True, cwd=SCRIPTS_DIR)
    subprocess.run(["git", "commit", "-am", "chore(releasing): generate changelog"],
                   check=True,
                   cwd=REPO_ROOT_DIR)

def create_branch(branch_name, dry_run=False):
    print(f"Creating branch: {branch_name}")
    subprocess.run(["git", "checkout", "-b", branch_name], check=True, cwd=REPO_ROOT_DIR)
    if not dry_run:
        subprocess.run(["git", "push", "-u", "origin", branch_name], check=True, cwd=REPO_ROOT_DIR)

def create_pull_request(branch_name, new_version, dry_run=False):
    title = f"Prepare {new_version} release"
    body = "Generated with the create-release-pull-request.py script."
    print(f"Creating pull request with title: {title}")
    if dry_run:
        print("Dry-run mode: Skipping PR creation.")
    else:
        try:
            subprocess.run(
                ["gh", "pr", "create", "--title", title, "--body", body, "--head", branch_name,
                 "--base", "main"], check=True, cwd=REPO_ROOT_DIR)
        except subprocess.CalledProcessError as e:
            print(f"Failed to create pull request: {e}")

def main():
    parser = argparse.ArgumentParser(description="Prepare a new release")
    parser.add_argument("version", help="The new version to release")
    parser.add_argument("--dry-run", action="store_true",
                        help="Run the script without making remote changes")
    args = parser.parse_args()

    new_version = args.version
    dry_run = args.dry_run

    validate_version(new_version)
    branch_name = f"prepare-{new_version}-release"
    create_branch(branch_name, dry_run)
    overwrite_version(new_version)
    generate_changelog()
    create_pull_request(branch_name, new_version, dry_run)

    if dry_run:
        print("Dry-run completed. No actual remote changes were made.")

if __name__ == "__main__":
    main()
