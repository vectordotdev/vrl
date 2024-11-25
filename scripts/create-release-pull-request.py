import os
import subprocess
import sys
from inspect import getsourcefile
from os.path import abspath

import semver
import toml

SCRIPTS_DIR = os.path.dirname(abspath(getsourcefile(lambda: 0)))
REPO_ROOT_DIR = os.path.dirname(SCRIPTS_DIR)
CHANGELOG_DIR = os.path.join(REPO_ROOT_DIR, "changelog.d")


def overwrite_version(version):
    toml_path = os.path.join(REPO_ROOT_DIR, "Cargo.toml")
    with open(toml_path, "r") as file:
        cargo_toml = toml.load(file)

    current_version = cargo_toml["package"]["version"]
    if current_version == version:
        print(f"Already at version {version}.")
        exit(1)

    commit_message = f"Overwrite version {current_version} with {version}"
    print(commit_message)

    cargo_toml["package"]["version"] = version
    with open(toml_path, "w") as file:
        toml.dump(cargo_toml, file)

    subprocess.run(["git", "commit", "-am", commit_message], check=True, cwd=REPO_ROOT_DIR)


def validate_version(version):
    try:
        semver.VersionInfo.parse(version)
    except ValueError:
        print(f"Invalid version: {version}. Please provide a valid SemVer string.")
        exit(1)


def generate_changelog():
    subprocess.run(["generate_release_changelog.sh"], check=True, cwd=SCRIPTS_DIR)
    subprocess.run(["git", "commit", "-am", "Generate changelog"], check=True, cwd=REPO_ROOT_DIR)


def create_branch(branch_name):
    subprocess.run(["git", "checkout", "-b", branch_name], check=True, cwd=REPO_ROOT_DIR)
    subprocess.run(["git", "push", "-u", "origin", branch_name], check=True, cwd=REPO_ROOT_DIR)


def create_pull_request(branch_name, new_version):
    title = f"Prepare {new_version} release"
    body = "Generated with the create-release-pull-request.py script."
    try:
        subprocess.run(
            ["gh", "pr", "create", "--title", title, "--body", body, "--head", branch_name,
             "--base", "main"], check=True, cwd=REPO_ROOT_DIR)
    except subprocess.CalledProcessError as e:
        print(f"Failed to create pull request: {e}")


def main():
    if len(sys.argv) != 2:
        print("Usage: script.py <version>")
        exit(1)
    new_version = sys.argv[1]
    validate_version(new_version)
    branch_name = f"prepare-{new_version}-release"
    create_branch(branch_name)
    overwrite_version(new_version)
    generate_changelog()
    create_pull_request(branch_name, new_version)


if __name__ == "__main__":
    main()
