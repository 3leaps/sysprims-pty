#!/usr/bin/env bash
set -euo pipefail

ROOT="$(git rev-parse --show-toplevel)"
VERSION="$(tr -d '\r\n' < "$ROOT/VERSION")"
TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/target}"
PACKAGE_DIR="$TARGET_DIR/package"
CRATE="$PACKAGE_DIR/sysprims-pty-${VERSION}.crate"
UNPACK_DIR="$(mktemp -d "${TMPDIR:-/tmp}/sysprims-pty-package.XXXXXX")"
trap 'rm -rf "$UNPACK_DIR"' EXIT HUP INT TERM

cd "$ROOT"

cargo_extra=()
if [ -n "$(git status --porcelain)" ]; then
	cargo_extra+=(--allow-dirty)
fi

python3 scripts/check-package-contents.py "${cargo_extra[@]}"
cargo package --locked "${cargo_extra[@]}"
cargo publish --dry-run --locked "${cargo_extra[@]}"

test -f "$CRATE" || {
	echo "error: expected crate archive not found: $CRATE" >&2
	exit 1
}

tar -xzf "$CRATE" -C "$UNPACK_DIR"
cd "$UNPACK_DIR/sysprims-pty-${VERSION}"

if grep -R "git+https://github.com/3leaps/sysprims" Cargo.lock Cargo.toml >/dev/null 2>&1; then
	echo "error: packaged crate contains git-sourced sysprims dependency" >&2
	exit 1
fi

CARGO_TARGET_DIR="$TARGET_DIR/package-unpacked" cargo test --all-targets --all-features

echo "[ok] package, dry-run publish, contents, and unpacked tests passed"
