## Overview

This directory contains changelog "fragments" that are collected during a release to
generate the project's changelog.

The changelog fragments are located in `changelog.d/`.

## Process

Fragments for unreleased changes are placed in the root of this directory alongside the changes they describe.

During a release, the `Prepare release` workflow runs `cargo run -p release`, which updates `CHANGELOG.md`.

### Submitting changes

By default, PRs are required to add at least one entry to this directory.
This is enforced during CI.

To mark a PR as not requiring changelog notes, add the label 'no-changelog'.

To run the same check that is run in CI to validate that your changelog fragments have
the correct syntax: `cargo run -p release -- check-changelog`

The format for fragments is: `<description>.<fragment_type>.md`

### Fragment conventions

When fragments are used to generate the updated changelog, the content of the fragment file is
rendered under the "type" of fragment.

The contents of the file must be valid markdown, followed by a required `authors:` line at the end.

Filename rules:

- The first segment is a short, unique description of the change and must not contain periods.
- The type must be one of the valid types in [Fragment types](#types)
- Only the two period delimiters can be used.
- The file must be markdown.

#### Fragment types {#types}

- breaking: A change that is incompatible with prior versions which requires users to make adjustments.
  If a change is also a fix or feature, breaking takes precedence.
- security: A change that has implications for security.
- deprecation: A change that is introducing a deprecation.
- feature: A change that is introducing a new feature.
- enhancement: A change that is enhancing existing functionality in a user perceivable way.
- fix: A change that is fixing a bug.

#### Fragment contents

Non-breaking fragments become items in a markdown list.
Their content must be renderable as a markdown list.
Breaking fragments follow the structured format in [Breaking changes](#breaking-changes).

As an example, separating content with markdown header syntax should be avoided, as that will render
as a heading in the main changelog and not the list. Instead, separate content with newlines.

#### Authors

Every fragment **must** end with an `authors:` line listing the GitHub username(s) of the contributor(s).
For non-breaking fragments, multiple authors are comma-separated.
Breaking fragments use Vector's space-delimited authors format (see below).
The authors are rendered as GitHub profile links in the changelog.

```
authors: github_username
```

or for multiple contributors:

```
authors: username1, username2
```

### Breaking changes

Breaking fragments (`*.breaking.md`) use the same format as
[Vector's breaking fragments](https://github.com/vectordotdev/vector/blob/master/changelog.d/README.md#breaking):

- Begin with an H1 title, optionally followed by a stable `{#anchor}`.
- Include exactly one `## Summary` and one `## Migration`, in that order.
- Put all content in those sections, with nothing between the title and `## Summary`.
- Use a nonempty Summary to explain what changed and who is affected.
- Describe required user actions in Migration. Before/After or Old/New examples are optional.
  Use `N/A` when no user action is required; an empty Migration is also accepted.
- End with `authors: <username> [<username> ...]`, with space-separated handles, no `@` or commas,
  and no trailing blank lines.
- Replace all `TODO` title/body placeholders and the `TODO_your_gh_handle` author placeholder.

Anchors default to the title converted to lowercase ASCII words separated by hyphens.
An explicit anchor must use lowercase ASCII letters, digits, and hyphens, with no leading or
trailing hyphen. Anchors must be nonempty and unique among pending breaking fragments.
As in Vector, `vector-breaking-changes`, `vector-upgrade-guide`, and `upgrade-guide` are reserved.

CI validates added and modified fragments, and release generation checks the same schema.
Generated changelogs preserve Summary and Migration, nest headings under the release's breaking
changes section, and prefix anchors with the VRL version to avoid collisions with earlier releases.
Existing published changelog entries are unchanged.

## Example

The following illustrates a hypothetical change to the default behavior of `parse_json`.

    $ cat changelog.d/parse-json-strict-default.breaking.md
    # Strict UTF-8 parsing by default {#parse-json-strict-default}

    ## Summary

    `parse_json` now rejects invalid UTF-8 by default instead of replacing invalid
    characters. Programs that parse messages containing invalid UTF-8 may now fail.

    ## Migration

    To preserve the previous behavior, pass `lossy: true` explicitly.

    #### Old

    ```vrl
    .parsed = parse_json!(.message)
    ```

    #### New

    ```vrl
    .parsed = parse_json!(.message, lossy: true)
    ```

    authors: your_github_username
