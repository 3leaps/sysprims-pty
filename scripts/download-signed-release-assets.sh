#!/usr/bin/env bash
set -euo pipefail

TAG=${1:?"usage: download-signed-release-assets.sh <tag> [dest_dir]"}
DEST=${2:-dist/release}
VERSION="${TAG#v}"

mkdir -p "$DEST"
find "$DEST" -mindepth 1 -maxdepth 1 -exec rm -r {} +

gh release download "$TAG" --dir "$DEST" --clobber \
	--pattern "sbom-${VERSION}.cdx.json" \
	--pattern 'LICENSE-MIT' \
	--pattern "release-notes-${TAG}.md" \
	--pattern 'SHA256SUMS' \
	--pattern 'SHA256SUMS.minisig' \
	--pattern 'SHA512SUMS' \
	--pattern 'SHA512SUMS.minisig' \
	--pattern 'sysprims-pty-minisign.pub' \
	--pattern 'SHA256SUMS.asc' \
	--pattern 'SHA512SUMS.asc' \
	--pattern 'sysprims-pty-release-signing-key.asc'

find "$DEST" -maxdepth 1 -type f -print | sort
