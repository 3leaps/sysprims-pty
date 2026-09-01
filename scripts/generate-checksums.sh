#!/usr/bin/env bash
set -euo pipefail

TAG=${1:?"usage: generate-checksums.sh <tag> [dir]"}
DIR=${2:-dist/release}
VERSION="${TAG#v}"
test -d "$DIR"

cd "$DIR"

required=(
	"LICENSE-MIT"
	"release-notes-${TAG}.md"
	"sbom-${VERSION}.cdx.json"
)

for file in "${required[@]}"; do
	test -f "$file" || {
		echo "error: required release asset missing: $file" >&2
		exit 1
	}
done

stale=$(
	find . -maxdepth 1 -type f \
		\( \
			\( -name 'release-notes-v*.md' ! -name "release-notes-${TAG}.md" \) -o \
			\( -name 'sbom-*.json' ! -name "sbom-${VERSION}.cdx.json" \) \
		\) \
		-print
)
if [ -n "$stale" ]; then
	echo "error: stale or unexpected versioned release assets present:" >&2
	printf '%s\n' "$stale" >&2
	exit 1
fi

find . -maxdepth 1 -type f \
	\( -name '*.json' -o -name 'LICENSE-*' -o -name 'release-notes-*.md' \) \
	! -name 'SHA*' \
	! -name '*.minisig' \
	! -name '*.asc' \
	! -name '*.pub' \
	-print0 | sort -z | xargs -0 shasum -a 256 > SHA256SUMS

find . -maxdepth 1 -type f \
	\( -name '*.json' -o -name 'LICENSE-*' -o -name 'release-notes-*.md' \) \
	! -name 'SHA*' \
	! -name '*.minisig' \
	! -name '*.asc' \
	! -name '*.pub' \
	-print0 | sort -z | xargs -0 shasum -a 512 > SHA512SUMS

echo "[ok] checksum manifests generated"
