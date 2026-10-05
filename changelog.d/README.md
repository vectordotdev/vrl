## Overview

This directory contains changelog "fragments" that are collected during a release to
generate the project's changelog.

The changelog fragments are located in `changelog.d/`.

## Process

Fragments for unreleased changes are placed in the root of this directory alongside the changes they describe.

During a release, `cargo run -p release` automatically updates `CHANGELOG.md`.

### Submitting changes

By default, PRs are required to add at least one entry to this directory.
This is enforced during CI.

To mark a PR as not requiring changelog notes, add the label 'no-changelog'.

To run the same check that is run in CI to validate that your changelog fragments have
the correct syntax: `cargo run -p release -- check-changelog`

The format for fragments is: `<description>.<fragment_type>.md`

### Fragment conventions

When fragments are used to generate the updated changelog, the content of the fragment file is
rendered as an item in a bulleted list under the "type" of fragment.

The contents of the file must be valid markdown, followed by a required `authors:` line at the end.

Filename rules:

- The first segment is a short, unique description of the change and must not contain periods.
- The type must be one of the valid types in [Fragment types](#types)
- Only the two period delimiters can be used.
- The file must be markdown.

#### Fragment types {#types}

- breaking: A change that is incompatible with prior versions which requires users to make adjustments.
- security: A change that has implications for security.
- deprecation: A change that is introducing a deprecation.
- feature: A change that is introducing a new feature.
- enhancement: A change that is enhancing existing functionality in a user perceivable way.
- fix: A change that is fixing a bug.

#### Fragment contents

When fragments are rendered in the changelog, each fragment becomes an item in a markdown list.
For this reason, when creating the content in a fragment, the format must be renderable as a markdown list.

As an example, separating content with markdown header syntax should be avoided, as that will render
as a heading in the main changelog and not the list. Instead, separate content with newlines.

#### Authors

Every fragment **must** end with an `authors:` line listing the GitHub username(s) of the contributor(s).
Multiple authors are comma-separated. The authors are rendered as GitHub profile links in the changelog.

```
authors: github_username
```

or for multiple contributors:

```
authors: username1, username2
```

### Breaking changes

Breaking fragments must explain what changed, who is affected, and how to migrate.
Include **Before** and **After** examples showing the required changes to VRL programs or
the change in output. If users do not need to change their programs, state that explicitly.

## Example

The following illustrates a hypothetical change to the default behavior of `parse_json`.

    $ cat changelog.d/parse-json-strict-default.breaking.md
    `parse_json` now rejects invalid UTF-8 by default instead of replacing invalid
    characters. Programs that parse messages containing invalid UTF-8 may now fail.

    To preserve the previous behavior, pass `lossy: true` explicitly.

    **Before:**

    ```vrl
    .parsed = parse_json!(.message)
    ```

    **After:**

    ```vrl
    .parsed = parse_json!(.message, lossy: true)
    ```

    authors: your_github_username
