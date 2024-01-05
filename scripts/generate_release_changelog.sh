#!/bin/bash

set -o errexit

README="README.md"
CHANGELOG="CHANGELOG.md"
CHANGELOG_DIR="changelog.d"
CHANGELOG_CFG="changelog.toml"

ask_continue() {
  while true; do
    local choice
    read -r choice
    case $choice in
      y) break; ;;
      n) exit 1; ;;
      *) echo "Please enter y or n"; ;;
    esac
  done
}

cd $(dirname "$0")/..
VRL_ROOT=$(pwd)

VRL_VERSION=$(awk '/^version = "[0-9]+.[0-9]+.[0-9]+"/{print $3}' "${VRL_ROOT}"/Cargo.toml | tr -d '"')

if ! [[ ${VRL_VERSION} =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] ; then
  echo "Error reading release version from Cargo.toml!"
  exit 1
else
  echo
  echo "[detected VRL release version: ${VRL_VERSION}]"
fi

if ! python3 -m pip show towncrier > /dev/null ; then
  echo "towncrier installation missing. Please install with 'python3 -m pip install towncrier'"
  exit 1
fi

LOCAL_CHANGELOG_DIR=${VRL_ROOT}/${CHANGELOG_DIR}

if [ ! -d "${LOCAL_CHANGELOG_DIR}" ]; then
  echo "No ${CHANGELOG_DIR} found in ${VRL_ROOT}!"
  exit 1
fi

LOCAL_CHANGELOG_CFG=${VRL_ROOT}/${CHANGELOG_CFG}

if [ ! -f "${LOCAL_CHANGELOG_CFG}" ]; then
  echo "No ${CHANGELOG_CFG} found in ${VRL_ROOT}!"
  exit 1
fi

################################################################################
echo
echo -n "[checking for changelog fragments..."

HAVE_FRAGMENTS=false

# changelog fragments that haven't been released are added at the root of the changelog dir.
for f in "${LOCAL_CHANGELOG_DIR}"/*.md ; do
  if [[ $(basename "$f") == "${README}" ]] ; then
    continue
  fi
  HAVE_FRAGMENTS=true
  echo " done.]"
  break
done

if [ "${HAVE_FRAGMENTS}" = false ] ; then
  echo " no changelog fragments were found! Exiting]"
  exit 1
fi

################################################################################

echo
echo "[generating the changelog..."
if python3 -m towncrier build \
  --config "${LOCAL_CHANGELOG_CFG}" \
  --dir "${VRL_ROOT}" \
  --version "${VRL_VERSION}" \
  --keep
then
  echo
  echo " done]"
else
  echo
  echo " failed!]"
  exit 1
fi

################################################################################
echo
echo "[please review the ${CHANGELOG} for accuracy.]"

echo
echo "[continue? The next step will retire changelog fragments being released by removing them from the repo (y/n)?]"

ask_continue

################################################################################
echo
echo -n "[removing changelog fragments included in this release..."

for f in "${LOCAL_CHANGELOG_DIR}"/*.md ; do
  if [[ $(basename "$f") == "${README}" ]] ; then
    continue
  fi
  if ! git rm -f "$f" ; then
    echo "... failed to remove $f !]"
    exit 1
  fi
done

echo " done]"

################################################################################
echo
echo "[please review the changes to local VRL repo checkout and create a PR.]"
