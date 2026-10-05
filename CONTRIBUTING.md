# Contributing

First, thank you for contributing to VRL! The goal of this document is to
provide everything you need to get started.

## Steps

1. If you are planning a large PR or a breaking change, or want feedback from us and the community, [open a new issue][urls.new_issue].
   Please check [existing issues][urls.existing_issues] to avoid duplicates.
2. Make your changes on a new branch (in your fork if contributing externally) and add or update tests to cover them.
3. Run `make all` to run tests and other checks. These checks also run in CI. Run `make help` to see all available targets.
4. If your PR changes user-facing function documentation, run `make generate-docs` to regenerate it. Do not edit `docs/generated/` directly.
5. For user-facing changes, add a changelog fragment to `changelog.d/`. See [changelog.d/README.md](changelog.d/README.md) for details.
6. [Submit the branch as a pull request][urls.submit_pr] to the repo, following [.github/PULL_REQUEST_TEMPLATE.md](.github/PULL_REQUEST_TEMPLATE.md).
7. A VRL maintainer will review your pull request.
8. It is normal to have multiple review iterations on a PR. To enable incremental reviews, please try to avoid force pushing if possible.
   - When a rebase is needed, try `git merge origin main` followed by `git push`.

[urls.existing_issues]: https://github.com/vectordotdev/vrl/issues
[urls.new_issue]: https://github.com/vectordotdev/vrl/issues/new
[urls.submit_pr]: https://help.github.com/en/github/collaborating-with-issues-and-pull-requests/creating-a-pull-request-from-a-fork
