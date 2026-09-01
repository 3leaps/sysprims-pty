#!/usr/bin/env bash
set -euo pipefail

TAG=${1:?"usage: upload-release-assets.sh <tag> [dir]"}
DIR=${2:-dist/release}
test -d "$DIR"

cd "$DIR"

required=(
	SHA256SUMS
	SHA256SUMS.minisig
	SHA512SUMS
	SHA512SUMS.minisig
	sysprims-pty-minisign.pub
	"release-notes-${TAG}.md"
)

for file in "${required[@]}"; do
	test -f "$file" || {
		echo "error: required upload file missing: $file" >&2
		exit 1
	}
done

uploads=("${required[@]}")
for optional in SHA256SUMS.asc SHA512SUMS.asc sysprims-pty-release-signing-key.asc; do
	if [ -f "$optional" ]; then
		uploads+=("$optional")
	fi
done

gh release upload "$TAG" "${uploads[@]}" --clobber
gh release edit "$TAG" --notes-file "release-notes-${TAG}.md"

echo "[ok] signed assets uploaded; release remains draft until an explicit undraft cue"
