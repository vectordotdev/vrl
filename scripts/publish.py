import os
import subprocess
from inspect import getsourcefile
from os.path import abspath

import requests
import toml

SCRIPTS_DIR = os.path.dirname(abspath(getsourcefile(lambda: 0)))
REPO_ROOT_DIR = os.path.dirname(SCRIPTS_DIR)


def get_crate_versions(crate_name):
    response = requests.get(f"https://crates.io/api/v1/crates/{crate_name}")
    if response.status_code != 200:
        raise Exception(f"Error fetching crate info: {response.status_code}")
    data = response.json()
    return [version["num"] for version in data["versions"]]


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
        subprocess.run(["git", "tag", "-a", tag_name, "-m", tag_message], check=True, cwd=REPO_ROOT_DIR)
        subprocess.run(["git", "push", "origin", version], check=True, cwd=REPO_ROOT_DIR)
        print(f"Tagged version.")
    except subprocess.CalledProcessError as e:
        print(f"An error occurred while publishing the crate: {e}")


def main():
    crate_name = "vrl"
    versions = get_crate_versions(crate_name)
    print(f"Available versions for {crate_name}: {versions}")

    toml_path = os.path.join(REPO_ROOT_DIR, "Cargo.toml")
    current_version = read_version_from_cargo_toml(toml_path)
    print(f"Current version in Cargo.toml: {current_version}")

    if current_version in versions:
        print(f"The version {current_version} is already published. Please update the version and try again.")
    else:
        publish_vrl(current_version)


if __name__ == "__main__":
    main()
