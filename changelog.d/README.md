## Overview

This directory contains changelog "fragments" that are collected during a release to
generate the project's changelog.

The conventions used for this changelog logic follow [towncrier](https://towncrier.readthedocs.io/en/stable/markdown.html).

The changelog fragments are located in `changelog.d/`.

## Process

Fragments for un-released changes are placed in the root of this directory during PRs.

During a release, `scripts/generate_release_changelog.sh` is run in order to automatically
generate the changes to the CHANGELOG.md file. As part of the script execution, the
changelog fragment files that are being released, are removed from the repo.

### Pull Requests

By default, PRs are required to add at least one entry to this directory.
This is enforced during CI.

To mark a PR as not requiring changelog notes, add the label 'no-changelog'.

To run the same check that is run in CI to validate that your changelog fragments have
the correct syntax, commit the fragment additions and then run `./scripts/check_changelog_fragments.sh`

The format for fragments is: `<pr_number>.<fragment_type>.md`

### Fragment conventions

When fragments used to generate the updated changelog, the content of the fragment file is
rendered as an item in a bulleted list under the "type" of fragment.

The contents of the file must be valid markdown.

Filename rules:

- The first segment (pr_number) must match the GitHub PR the change is introduced in.
- The type must be one of the valid types in [Fragment types](#types)
- Only the two period delimiters can be used.
- The file must be markdown.

#### Fragment types {#types}

- breaking: A change that is incompatible with prior versions which requires users to make adjustments.
- security: A change that is has implications for security.
- deprecation: A change that is introducing a deprecation.
- feature: A change that is introducing a new feature.
- enhancement: A change that is enhancing existing functionality in a user perceivable way.
- fix: A change that is fixing a bug.

#### Fragment contents

When fragments are rendered in the changelog, each fragment becomes an item in a markdown list.
For this reason, when creating the content in a fragment, the format must be renderable as a markdown list.

As an example, separating content with markdown header syntax should be avoided, as that will render
as a heading in the main changelog and not the list. Instead, separate content with newlines.

### Breaking changes

When using the type 'breaking' to add notes for a breaking change, these should be more verbose than
other entries typically. It should include all details that would be relevant for the user to need
to handle upgrading to the breaking change.

## Example

Here is an example of a changelog fragment that adds a breaking change explanation.

    $ cat changelog.d/42.breaking.md
    This change is so great. It's such a great change that this sentence
    explaining the change has to span multiple lines of text.

    It even necessitates a line break. It is a breaking change after all.
