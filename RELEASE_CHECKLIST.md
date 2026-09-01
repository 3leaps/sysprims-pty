# Release checklist

This repository publishes a Rust library crate only. Do not add platform
archives, FFI bundles, Go modules, npm packages, or automated crates.io
publication.

## Prepare

- [ ] Confirm `VERSION`, `Cargo.toml`, `CHANGELOG.md`, and
      `docs/releases/vX.Y.Z.md` agree.
- [ ] Run `make release-preflight` from a clean `main` synchronized with
      `origin/main`.
- [ ] Open and merge the release-prep PR after review assent.

## Tag

```bash
VERSION=$(cat VERSION)
git switch main
git pull --ff-only origin main
make release-preflight
git tag -a "v${VERSION}" -m "v${VERSION}"
git push origin "v${VERSION}"
```

The tag workflow must create a draft GitHub release only. It must not sign
assets and must not publish to crates.io.

## First crates.io publication

Publication is irreversible and requires a separate explicit maintainer cue.

```bash
VERSION=$(cat VERSION)
git checkout --detach "v${VERSION}"
SYSPRIMS_PTY_REQUIRE_TAG=1 make release-guard-tag-version
cargo info --registry crates-io sysprims-pty
cargo publish --dry-run --locked
cargo publish --locked
cargo info --registry crates-io "sysprims-pty@${VERSION}"
```

Stop if the crate name resolves to an unexpected owner or if the dry-run
tarball differs from the reviewed package.

## Sign and upload release provenance

After the draft GitHub release exists and the crates.io publish cue has been
handled:

```bash
export SYSPRIMS_PTY_RELEASE_TAG=v$(cat VERSION)
make release-download
make release-checksums
make release-sign
make release-export-keys
make release-verify
make release-upload
```

Publishing the GitHub release is a separate explicit maintainer cue:

```bash
gh release edit "v$(cat VERSION)" --draft=false
```

## Post-release verification

- [ ] `vX.Y.Z` is annotated and peels to the reviewed commit.
- [ ] GitHub release is public and contains notes, license, SBOM, checksums,
      signatures, and public keys.
- [ ] `make release-verify` passes from downloaded assets.
- [ ] `cargo info --registry crates-io sysprims-pty@X.Y.Z` resolves.
- [ ] docs.rs recognizes the published version.
- [ ] A registry-only temporary consumer builds with:

  ```toml
  [dependencies]
  sysprims-pty = "=X.Y.Z"
  ```

  and imports `portable_pty`.
