#!/usr/bin/env python3
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SEMVER = re.compile(r"^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$")


def fail(message: str) -> None:
    print(f"error: {message}", file=sys.stderr)
    raise SystemExit(1)


def read(path: str) -> str:
    return (ROOT / path).read_text(encoding="utf-8")


version_file = read("VERSION")
if not version_file.endswith("\n") or version_file.count("\n") != 1:
    fail("VERSION must contain exactly one SemVer followed by LF")

version = version_file.strip()
if not SEMVER.fullmatch(version):
    fail(f"VERSION is not SemVer: {version}")

cargo = read("Cargo.toml")
package = re.search(r'(?m)^version = "([^"]+)"$', cargo)
if not package:
    fail("Cargo.toml package version not found")
if package.group(1) != version:
    fail(f"Cargo.toml version {package.group(1)} does not match VERSION {version}")

required = {
    'sysprims-timeout = "=0.2.3"': "sysprims-timeout registry dependency",
    'version = "=0.2.3"': "sysprims-session registry dependency",
    "publish = true": "publish flag",
}
for needle, label in required.items():
    if needle not in cargo:
        fail(f"missing {label}: {needle}")

if "git =" in cargo:
    fail("Cargo.toml must not contain git dependencies for release packaging")
if "Cargo.toml.orig" in cargo:
    fail("Cargo.toml must not reference Cargo.toml.orig")
if (ROOT / "Cargo.toml.orig").exists():
    fail("Cargo.toml.orig must not exist")

for path in ("CHANGELOG.md", "RELEASE_NOTES.md", f"docs/releases/v{version}.md"):
    if not (ROOT / path).is_file():
        fail(f"missing release document: {path}")

lock = read("Cargo.lock")
if "git+https://github.com/3leaps/sysprims" in lock:
    fail("Cargo.lock still contains git-sourced sysprims packages")
for crate in ("sysprims-session", "sysprims-timeout"):
    pattern = re.compile(
        rf'(?s)\[\[package\]\]\nname = "{re.escape(crate)}"\nversion = "([^"]+)"\nsource = "registry\+https://github.com/rust-lang/crates.io-index"'
    )
    match = pattern.search(lock)
    if not match:
        fail(f"Cargo.lock missing registry source for {crate}")
    if match.group(1) != "0.2.3":
        fail(f"Cargo.lock has {crate} {match.group(1)}, expected 0.2.3")

print(f"[ok] version pack is coherent for v{version}")
