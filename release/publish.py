#!/usr/bin/env python3
import os
import subprocess
from inspect import getsourcefile
from os.path import abspath

import toml

from utils.validate_version import assert_version_is_not_published

SCRIPTS_DIR = os.path.dirname(abspath(getsourcefile(lambda: 0)))
REPO_ROOT_DIR = os.path.dirname(SCRIPTS_DIR)
CHANGELOG_DIR = os.path.join(REPO_ROOT_DIR, "changelog.d")


def read_version_from_cargo_toml(filepath):
    with open(filepath, "r") as file:
        cargo_toml = toml.load(file)
        return cargo_toml["package"]["version"]


def publish_vrl(version):
    try:
        subprocess.run(["cargo", "publish"], check=True, cwd=REPO_ROOT_DIR)
        print(f"Published VRL v{version}.")

        tag_name = f"v{version}"
        tag_message = f"Release {version}"
        subprocess.run(["git", "tag", "-a", tag_name, "-m", tag_message], check=True,
                       cwd=REPO_ROOT_DIR)
        subprocess.run(["git", "push", "origin", tag_name], check=True, cwd=REPO_ROOT_DIR)
        print(f"Tagged version.")
    except subprocess.CalledProcessError as e:
        print(f"An error occurred while publishing the crate: {e}")


def assert_no_changelog_fragments():
    entries = os.listdir(CHANGELOG_DIR)
    error = f"{CHANGELOG_DIR} should only contain a README.md file. Did you run ../scripts/generate_release_changelog.sh?"
    assert len(entries) == 1, error
    assert entries[0] == "README.md", error


def main():
    assert_no_changelog_fragments()

    toml_path = os.path.join(REPO_ROOT_DIR, "Cargo.toml")
    current_version = read_version_from_cargo_toml(toml_path)
    print(f"Current version in Cargo.toml: {current_version}")
    assert_version_is_not_published(current_version)
    publish_vrl(current_version)


if __name__ == "__main__":
    main()
