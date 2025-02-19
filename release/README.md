# VRL release steps

1. Create a release PR

```shell
python3 -m release.create_release_pull_request <version>
```

2. Wait for the PR to be merged

3. Run the publish script

```shell
python3 -m release.publish.py
```
