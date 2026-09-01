#!/usr/bin/env bash
set -euo pipefail

DIR=${1:-dist/release}
test -d "$DIR"

cd "$DIR"

errors=0

if [ ! -f sysprims-pty-minisign.pub ]; then
	echo "error: missing required sysprims-pty-minisign.pub" >&2
	errors=$((errors + 1))
elif grep -qi "secret" sysprims-pty-minisign.pub; then
	echo "error: minisign public key file appears to contain secret material" >&2
	errors=$((errors + 1))
elif ! grep -q "^untrusted comment:" sysprims-pty-minisign.pub; then
	echo "error: minisign public key has unexpected format" >&2
	errors=$((errors + 1))
fi

if [ -f SHA256SUMS.asc ] || [ -f SHA512SUMS.asc ]; then
	if [ ! -f sysprims-pty-release-signing-key.asc ]; then
		echo "error: PGP signatures exist but sysprims-pty-release-signing-key.asc is missing" >&2
		errors=$((errors + 1))
	fi
fi

if [ -f sysprims-pty-release-signing-key.asc ]; then
	if grep -q "PRIVATE KEY BLOCK" sysprims-pty-release-signing-key.asc; then
		echo "error: PGP export contains private key material" >&2
		errors=$((errors + 1))
	fi
	if ! grep -q "PUBLIC KEY BLOCK" sysprims-pty-release-signing-key.asc; then
		echo "error: PGP export has unexpected format" >&2
		errors=$((errors + 1))
	fi
fi

test "$errors" -eq 0
echo "[ok] exported keys are public-only"
