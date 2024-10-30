# Contributing

First, thank you for contributing to VRL! The goal of this document is to
provide everything you need to get started.

## Steps

1. Ensure your change has an issue!
   Find an [existing issue][urls.existing_issues] or [open a new issue][urls.new_issue].
2. [fork the VRL repository][urls.fork_repo] in your own
   GitHub account (only applicable to outside contributors).
3. [Create a new Git branch][urls.create_branch].
4. Make your changes.
5. Add and/or update tests to cover your changes.
6. Run `./scripts/checks.sh` to run tests and other checks.
7. [Submit the branch as a pull request][urls.submit_pr] to the repo. A team member should
   comment and/or review your pull request.
8. Add a changelog fragment (requires the PR number) to describe your changes which will
   be included in the release changelog. See the [README.md](changelog.d/README.md) for details.

[urls.existing_issues]: https://github.com/vectordotdev/vrl/issues
[urls.new_issue]: https://github.com/vectordotdev/vrl/issues/new
[urls.create_branch]: https://help.github.com/en/github/collaborating-with-issues-and-pull-requests/creating-and-deleting-branches-within-your-repository
[urls.fork_repo]: https://help.github.com/en/github/getting-started-with-github/fork-a-repo
[urls.submit_pr]: https://help.github.com/en/github/collaborating-with-issues-and-pull-requests/creating-a-pull-request-from-a-fork
