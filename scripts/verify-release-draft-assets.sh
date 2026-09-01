#!/usr/bin/env bash
set -euo pipefail

TAG=${1:?"usage: verify-release-draft-assets.sh <tag> <unsigned|signed>"}
MODE=${2:?"usage: verify-release-draft-assets.sh <tag> <unsigned|signed>"}
VERSION="${TAG#v}"

is_draft="$(gh release view "$TAG" --json isDraft --jq '.isDraft')"
if [ "$is_draft" != "true" ]; then
	echo "error: release ${TAG} is not draft" >&2
	exit 1
fi

case "$MODE" in
unsigned)
	required=(
		"LICENSE-MIT"
		"release-notes-${TAG}.md"
		"sbom-${VERSION}.cdx.json"
	)
	allowed="^(LICENSE-MIT|release-notes-${TAG}\.md|sbom-${VERSION}\.cdx\.json)$"
	;;
signed)
	required=(
		"LICENSE-MIT"
		"release-notes-${TAG}.md"
		"sbom-${VERSION}.cdx.json"
		"SHA256SUMS"
		"SHA256SUMS.minisig"
		"SHA512SUMS"
		"SHA512SUMS.minisig"
		"sysprims-pty-minisign.pub"
	)
	allowed="^(LICENSE-MIT|release-notes-${TAG}\.md|sbom-${VERSION}\.cdx\.json|SHA256SUMS|SHA256SUMS\.minisig|SHA256SUMS\.asc|SHA512SUMS|SHA512SUMS\.minisig|SHA512SUMS\.asc|sysprims-pty-minisign\.pub|sysprims-pty-release-signing-key\.asc)$"
	;;
*)
	echo "error: unknown asset verification mode: $MODE" >&2
	exit 1
	;;
esac

assets="$(gh release view "$TAG" --json assets --jq '.assets[].name')"
for file in "${required[@]}"; do
	if ! printf '%s\n' "$assets" | grep -Fxq "$file"; then
		echo "error: release ${TAG} is missing asset ${file}" >&2
		echo "$assets" >&2
		exit 1
	fi
done

if printf '%s\n' "$assets" | grep -Ev "$allowed" >/dev/null; then
	echo "error: release ${TAG} has unexpected assets for ${MODE} mode" >&2
	echo "$assets" >&2
	exit 1
fi

echo "[ok] release ${TAG} is draft with expected ${MODE} assets"
