#!/bin/sh
set -eu

companion_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
sysprims_root=${SYSPRIMS_ROOT:-"$companion_root/../sysprims"}
reviewed_sysprims_rev=e366d37bbdbe28764c0f7022577b1999393742cb

if ! docker info >/dev/null 2>&1; then
    echo "docker daemon unavailable; start the reviewed disposable runtime" >&2
    exit 2
fi

test "$(git -C "$sysprims_root" rev-parse HEAD)" = "$reviewed_sysprims_rev"
test -z "$(git -C "$sysprims_root" status --short)"

docker run --rm \
    --env SYSPRIMS_PTY_DISPOSABLE=1 \
    --volume "$companion_root:/work/companion:ro" \
    --volume "$sysprims_root:/work/sysprims:ro" \
    rust:1.88-bookworm \
    sh -eu -c '
        cp -R /work/companion /tmp/companion
        cd /tmp/companion
        sed -i \
          -e "s|^sysprims-timeout = .*|sysprims-timeout = { path = \"/work/sysprims/crates/sysprims-timeout\" }|" \
          -e "/^\[target.\"cfg(unix)\".dependencies.sysprims-session\]$/,\$c\\
[target.\"cfg(unix)\".dependencies.sysprims-session]\\
path = \"/work/sysprims/crates/sysprims-session\"\\
" \
          Cargo.toml
        cargo test --test diabolical -- --ignored --test-threads=1
    '
