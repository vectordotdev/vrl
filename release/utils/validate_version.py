import requests


def get_crate_versions(crate_name):
    # crates.io returns a 403 now for the default requests user-agent
    headers = {
        'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/111.0.0.0 Safari/537.36'}

    response = requests.get(f"https://crates.io/api/v1/crates/{crate_name}", headers=headers)
    if response.status_code != 200:
        raise Exception(f"Error fetching crate info: {response.status_code}")
    data = response.json()
    return [version["num"] for version in data["versions"]]


def assert_version_is_not_published(current_version):
    crate_name = "vrl"
    versions = get_crate_versions(crate_name)
    print(f"Available versions for {crate_name}: {versions}")

    if current_version in versions:
        print(
            f"The version {current_version} is already published. Please update the version and try again.")
        exit(1)
