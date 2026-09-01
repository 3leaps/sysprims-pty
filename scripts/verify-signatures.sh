#!/usr/bin/env bash
set -euo pipefail

DIR=${1:-dist/release}
test -d "$DIR"

cd "$DIR"

errors=0

if [ ! -f sysprims-pty-minisign.pub ]; then
	echo "error: missing sysprims-pty-minisign.pub" >&2
	errors=$((errors + 1))
else
	for manifest in SHA256SUMS SHA512SUMS; do
		if [ -f "$manifest" ] && [ -f "${manifest}.minisig" ]; then
			minisign -Vm "$manifest" -p sysprims-pty-minisign.pub || errors=$((errors + 1))
		else
			echo "error: missing $manifest or ${manifest}.minisig" >&2
			errors=$((errors + 1))
		fi
	done
fi

pgp_artifacts=0
for file in sysprims-pty-release-signing-key.asc SHA256SUMS.asc SHA512SUMS.asc; do
	if [ -f "$file" ]; then
		pgp_artifacts=1
	fi
done

if [ "$pgp_artifacts" -eq 1 ]; then
	for file in sysprims-pty-release-signing-key.asc SHA256SUMS.asc SHA512SUMS.asc; do
		if [ ! -f "$file" ]; then
			echo "error: incomplete PGP signature set; missing $file" >&2
			errors=$((errors + 1))
		fi
	done
fi

if [ -f sysprims-pty-release-signing-key.asc ] && [ -f SHA256SUMS.asc ] && [ -f SHA512SUMS.asc ]; then
	GNUPGHOME="$(mktemp -d)"
	export GNUPGHOME
	trap 'rm -rf "$GNUPGHOME"' EXIT
	gpg --import sysprims-pty-release-signing-key.asc >/dev/null 2>&1
	for manifest in SHA256SUMS SHA512SUMS; do
		if [ -f "${manifest}.asc" ]; then
			gpg --verify "${manifest}.asc" "$manifest" >/dev/null 2>&1 || errors=$((errors + 1))
		fi
	done
fi

test "$errors" -eq 0
echo "[ok] release signatures verified"
