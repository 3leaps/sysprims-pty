#!/usr/bin/env bash
set -euo pipefail

ROOT="$(git rev-parse --show-toplevel)"
VERSION="$(tr -d '\r\n' < "$ROOT/VERSION")"
WORK_DIR="$(mktemp -d "${TMPDIR:-/tmp}/sysprims-pty-consumer.XXXXXX")"
cleanup() {
	if [ -n "${WORK_DIR:-}" ] && [ -d "$WORK_DIR" ]; then
		find "$WORK_DIR" -mindepth 1 -maxdepth 1 -exec rm -r {} +
		rmdir "$WORK_DIR"
	fi
}
trap cleanup EXIT HUP INT TERM

cd "$WORK_DIR"
cargo init --bin --quiet

cat > Cargo.toml <<EOF
[package]
name = "sysprims-pty-consumer-smoke"
version = "0.0.0"
edition = "2021"
publish = false

[dependencies]
sysprims-pty = "=${VERSION}"
EOF

cat > src/main.rs <<'EOF'
use portable_pty::CommandBuilder;

fn main() {
    let _cmd = CommandBuilder::new("true");
}
EOF

cargo generate-lockfile
cargo build --locked
cargo tree --locked | grep -F "sysprims-pty v${VERSION}"

if cargo tree --locked | grep -F "git+"; then
	echo "error: registry consumer resolved a git dependency" >&2
	exit 1
fi

echo "[ok] registry-only consumer builds with sysprims-pty ${VERSION}"
