#!/usr/bin/env python3
import re
import subprocess
import sys
from pathlib import Path

extra_args = sys.argv[1:]
root = Path(__file__).resolve().parents[1]
version = (root / "VERSION").read_text(encoding="utf-8").strip()
allowed = re.compile(
    r"^("
    r"\.cargo_vcs_info\.json|"
    r"Cargo\.(toml|lock)|"
    r"Cargo\.toml\.orig|"
    r"README\.md|UPSTREAM\.md|LICENSE-MIT|CHANGELOG\.md|RELEASE_NOTES\.md|VERSION|"
    rf"docs/releases/v{re.escape(version)}\.md|"
    r"src/.*\.rs|tests/.*\.rs|examples/.*\.rs"
    r")$"
)

result = subprocess.run(
    ["cargo", "package", "--list", "--locked", *extra_args],
    check=True,
    text=True,
    stdout=subprocess.PIPE,
)

bad = []
for raw in result.stdout.splitlines():
    path = raw.strip()
    if not path:
        continue
    if path.startswith("/") or ".." in path.split("/"):
        bad.append(path)
        continue
    if not allowed.match(path):
        bad.append(path)

if bad:
    print("error: unexpected package entries:", file=sys.stderr)
    for path in bad:
        print(f"  {path}", file=sys.stderr)
    raise SystemExit(1)

print("[ok] package contents match allowlist")
