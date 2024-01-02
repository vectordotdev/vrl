#!/bin/bash

# This script is intended to run during CI, however it can be run locally by
# committing changelog fragments before executing the script. If the script
# finds an issue with your changelog fragment, you can un-stage the fragment
# from being committed and fix the issue.

CHANGELOG_DIR="changelog.d"

# NOTE: If these are altered, update both the 'changelog.d/README.md' and
#       'scripts/generate_release_chanbgelog.sh' accordingly.
FRAGMENT_TYPES="breaking|security|deprecation|feature|enhancement|fix"

if [ ! -d "${CHANGELOG_DIR}" ]; then
  echo "No ./${CHANGELOG_DIR} found. This tool must be invoked from the root of the repo."
  exit 1
fi

# diff-filter=A lists only added files
FRAGMENTS=$(git diff --name-only --diff-filter=A origin/main ${CHANGELOG_DIR})

if [ "$(echo "$FRAGMENTS" | grep -c .)" -lt 1 ]; then
  echo "No changelog fragments detected"
  echo "If no changes  necessitate user-facing explanations, add the GH label 'no-changelog'"
  echo "Otherwise, add changelog fragments to changelog.d/"
  echo "For details, see 'changelog.d/README.md'"
  exit 1
fi

# extract the basename from the file path
FRAGMENTS=$(xargs -n1 basename <<< "${FRAGMENTS}")

# validate the fragments
while IFS= read -r fname; do

  if [[ ${fname} == "README.md" ]]; then
    continue
  fi

  echo "validating '${fname}'"

  IFS="." read -r -a arr <<< "${fname}"

  if [ "${#arr[@]}" -ne 3 ]; then
    echo "invalid fragment filename: wrong number of period delimiters. expected '<pr_number>.<fragment_type>.md'. (${fname})"
    exit 1
  fi

  if ! [[ "${arr[0]}" =~ ^[0-9]+$ ]]; then
    echo "invalid fragment filename: first segment must be PR number. (${fname})"
    exit 1
  fi

  if ! [[ "${arr[1]}" =~ ^(${FRAGMENT_TYPES})$ ]]; then
    echo "invalid fragment filename: fragment type must be one of: (${FRAGMENT_TYPES}). (${fname})"
    exit 1
  fi

  if [[ "${arr[2]}" != "md" ]]; then
    echo "invalid fragment filename: extension must be markdown (.md): (${fname})"
    exit 1
  fi

done <<< "$FRAGMENTS"

echo "changelog additions are valid."
