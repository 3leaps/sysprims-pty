#!/bin/sh
set -eu

companion_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
sysprims_root=${SYSPRIMS_ROOT:-"$companion_root/../sysprims"}
reviewed_sysprims_rev=419835f84466cb2e0b1ef9a9ed0592dcb3c4c4c8

actual_sysprims_rev=$(git -C "$sysprims_root" rev-parse HEAD)
if [ "$actual_sysprims_rev" != "$reviewed_sysprims_rev" ]; then
    echo "sysprims must be at reviewed revision $reviewed_sysprims_rev" >&2
    echo "found $actual_sysprims_rev" >&2
    exit 2
fi
if [ -n "$(git -C "$sysprims_root" status --short)" ]; then
    echo "sysprims worktree must be clean" >&2
    exit 2
fi

work_root=$(mktemp -d "${TMPDIR:-/tmp}/sysprims-pty-candidate.XXXXXX")
trap 'rm -rf "$work_root"' EXIT HUP INT TERM

mkdir -p "$work_root/companion"
tar -C "$companion_root" \
    --exclude .git \
    --exclude target \
    -cf - . | tar -C "$work_root/companion" -xf -

awk \
    -v timeout_path="$sysprims_root/crates/sysprims-timeout" \
    -v session_path="$sysprims_root/crates/sysprims-session" \
    '
    $0 == "[dependencies.sysprims-timeout]" {
        print
        print "path = \"" timeout_path "\""
        replacing_timeout = 1
        next
    }
    replacing_timeout {
        if ($0 == "") {
            print
            replacing_timeout = 0
        }
        next
    }
    $0 == "[target.\"cfg(unix)\".dependencies.sysprims-session]" {
        print
        print "path = \"" session_path "\""
        replacing_session = 1
        next
    }
    replacing_session { next }
    { print }
    ' "$work_root/companion/Cargo.toml" >"$work_root/Cargo.toml"
mv "$work_root/Cargo.toml" "$work_root/companion/Cargo.toml"

echo "sysprims candidate: $reviewed_sysprims_rev"
cd "$work_root/companion"
CARGO_TARGET_DIR="$companion_root/target/candidate-check" \
    cargo fmt --all -- --check
CARGO_TARGET_DIR="$companion_root/target/candidate-check" \
    cargo clippy --all-targets --all-features -- -D warnings
CARGO_TARGET_DIR="$companion_root/target/candidate-check" \
    cargo test --all-targets --all-features
CARGO_TARGET_DIR="$companion_root/target/candidate-check-windows-x64" \
    RUSTFLAGS=-Dwarnings \
    cargo check --all-targets --all-features --target x86_64-pc-windows-msvc
CARGO_TARGET_DIR="$companion_root/target/candidate-check-windows-arm64" \
    RUSTFLAGS=-Dwarnings \
    cargo check --all-targets --all-features --target aarch64-pc-windows-msvc
