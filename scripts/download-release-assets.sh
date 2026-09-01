#!/usr/bin/env bash
set -euo pipefail

TAG=${1:?"usage: download-release-assets.sh <tag> [dest_dir]"}
DEST=${2:-dist/release}

mkdir -p "$DEST"
find "$DEST" -mindepth 1 -maxdepth 1 -exec rm -r {} +
gh release download "$TAG" --dir "$DEST" --clobber \
	--pattern 'sbom-*.json' \
	--pattern 'LICENSE-*' \
	--pattern 'release-notes-*.md'

find "$DEST" -maxdepth 1 -type f -print | sort
