# Agent instructions

For development work in this repository, follow [CONTRIBUTING.md](CONTRIBUTING.md) and [AI_POLICY.md](AI_POLICY.md).

## Backwards Compatibility

- We strive to preserve backwards compatibility for VRL programs, including existing functions, their arguments, and behavior.
- We do not currently promise backwards compatibility for developers using VRL as a Rust library.

## Git Conventions

- **Commit messages:** Do NOT include co-authoring information from coding agents (i.e. avoid "Co-Authored-By: Claude" attribution).
- **Pull requests:** Do NOT add "Generated with Claude Code" or similar footers — keep PR descriptions focused on the technical changes.

### Preserve Open Pull Request History

Before rewriting a branch that has been pushed, use `gh` when available to check whether the branch has an open pull request:

```sh
gh pr list --head "$(git branch --show-current)" --state open --json number,url
```

If `gh` is unavailable or the check fails, assume an open pull request exists.

When an open pull request exists, never rewrite published commits or force-push the branch. Push additional commits normally to preserve incremental review.

## Creating Pull Requests

Before opening a PR, read [`.github/PULL_REQUEST_TEMPLATE.md`](.github/PULL_REQUEST_TEMPLATE.md) and use it as the reference for the PR body structure and title.
