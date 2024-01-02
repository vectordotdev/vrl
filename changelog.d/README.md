## Overview

This directory contains changelog "fragments" that are collected during a release to
generate the project's changelog.

The conventions used for this changelog logic follow [towncrier](https://towncrier.readthedocs.io/en/stable/markdown.html).

The changelog fragments are located in `changelog.d/`.

## Process

Fragments for un-released changes are placed in the root of this directory during PRs.

During a release when the changelog is generated, the fragments in the root of this
directory are moved into a new directory with the name of the release (e.g. '0.10.0').

### Pull Requests

By default, PRs are required to add at least one entry to this directory.
This is enforced during CI.

To mark a PR as not requiring changelog notes, add the label 'no-changelog'.

To run the same check that is run in CI to validate that your changelog fragments have
the correct syntax, commit the fragment additions and then run ./scripts/check_changelog_fragments.sh

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

### Breaking changes

When using the type 'breaking' to add notes for a breaking change, these should be more verbose than
other entries typically. It should include all details that would be relevant for the user to need
to handle upgrading to the breaking change.

## Community Contributors

When a PR is authored/has commits by a contributor from the VRL community, the fragment contents
can optionally contain a line which specifies the community members involved in making the change.
This is later used during the release process to render as a link to the github user profile for
the authors specified.

The process for adding this is simply to have the last line of the file be in this format:

    authors: <author1_gh_username>, <author2_gh_username>, <...>

## Example

Here is an example of a changelog fragment that adds a breaking change explanation.

    $ cat changelog.d/42_very_good_words.breaking.md
    This change is so great. It's such a great change that this sentence
    explaining the change has to span multiple lines of text.

    It even necessitates a line break. It is a breaking change after all.
