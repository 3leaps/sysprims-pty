#!/usr/bin/env bash
set -euo pipefail

TAG=${1:?"usage: sign-release-assets.sh <tag> [dir]"}
DIR=${2:-dist/release}

test -d "$DIR"

if [ -z "${SYSPRIMS_PTY_MINISIGN_KEY:-}" ]; then
	echo "error: SYSPRIMS_PTY_MINISIGN_KEY is required" >&2
	exit 1
fi
test -f "$SYSPRIMS_PTY_MINISIGN_KEY"

cd "$DIR"

for manifest in SHA256SUMS SHA512SUMS; do
	test -f "$manifest"
	minisign -S -s "$SYSPRIMS_PTY_MINISIGN_KEY" \
		-m "$manifest" \
		-t "sysprims-pty $TAG - $(date -u +%Y-%m-%dT%H:%M:%SZ)" \
		-x "${manifest}.minisig"
done

if [ -n "${SYSPRIMS_PTY_PGP_KEY_ID:-}" ]; then
	GPG_OPTS=()
	if [ -n "${SYSPRIMS_PTY_GPG_HOMEDIR:-}" ]; then
		GPG_OPTS+=("--homedir" "$SYSPRIMS_PTY_GPG_HOMEDIR")
	fi
	for manifest in SHA256SUMS SHA512SUMS; do
		gpg "${GPG_OPTS[@]}" --armor --detach-sign \
			--local-user "$SYSPRIMS_PTY_PGP_KEY_ID" \
			--output "${manifest}.asc" \
			"$manifest"
	done
fi

echo "[ok] release manifests signed"
