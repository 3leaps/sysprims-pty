#!/usr/bin/env bash
set -euo pipefail

DIR=${1:-dist/release}
test -d "$DIR"

MINISIGN_PUB="${SYSPRIMS_PTY_MINISIGN_PUB:-}"
if [ -z "$MINISIGN_PUB" ] && [ -n "${SYSPRIMS_PTY_MINISIGN_KEY:-}" ]; then
	MINISIGN_PUB="${SYSPRIMS_PTY_MINISIGN_KEY%.key}.pub"
fi

if [ -n "$MINISIGN_PUB" ] && [ -f "$MINISIGN_PUB" ]; then
	cp "$MINISIGN_PUB" "$DIR/sysprims-pty-minisign.pub"
else
	echo "error: minisign public key not found; set SYSPRIMS_PTY_MINISIGN_PUB" >&2
	exit 1
fi

if [ -n "${SYSPRIMS_PTY_PGP_KEY_ID:-}" ]; then
	GPG_OPTS=()
	if [ -n "${SYSPRIMS_PTY_GPG_HOMEDIR:-}" ]; then
		GPG_OPTS+=("--homedir" "$SYSPRIMS_PTY_GPG_HOMEDIR")
	fi
	gpg "${GPG_OPTS[@]}" --armor --export "$SYSPRIMS_PTY_PGP_KEY_ID" \
		>"$DIR/sysprims-pty-release-signing-key.asc"
fi

echo "[ok] public keys exported"
