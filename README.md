# sysprims-pty

`sysprims-pty` is a compatibility release of `portable-pty` 0.9.0 with a
safe, spawn-time sysprims containment path on Unix.

The normal portable-pty API remains available. On macOS and Linux,
`SlavePty::spawn_contained_command` installs the prepared sysprims acquisition
hook in the PTY-owned spawn, validates its sealed same-spawn receipt, and
returns an owned `ContainmentGuard<ContainedPtyChild>`.

## Containment semantics

- `TreeKillReliability::Guaranteed` means race-free session/group acquisition
  and group-signaling eligibility.
- The guard exclusively owns child observation and reap through a terminal
  lifecycle transition.
- Completion evidence is reported independently as `Empty`, `Survivors`, or
  `Unknown`.
- A cooperative Unix descendant can still leave its acquired group. Guaranteed
  acquisition is not an OS-enforced non-escape guarantee.
- Unsupported implementations, including Windows, reject this guarded API
  before spawning.

## Owned real-PTY examples

The integration tests contain two small, executable public-API examples. Each
opens a real controlling PTY, starts at least one descendant, and proves
`Guaranteed` acquisition, supported `Empty` completion, and exact leader reap:

1. [`owned_empty_explicit_close_with_descendant`](tests/contained_spawn.rs)
   calls `ContainmentGuard::terminate`.
2. [`owned_empty_natural_leader_exit_with_descendant`](tests/contained_spawn.rs)
   polls `ContainmentGuard::try_complete`, retrying only `Ok(None)` while the
   leader is running. Any error fails the scene.

Run both on demand:

```console
make test-owned-pty-empty
```

The runner uses the sibling `../sysprims` checkout by default, verifies its
exact reviewed revision and clean worktree, then applies it as a local source
override in an isolated copy. Set `SYSPRIMS_ROOT` to use another checkout.
The package manifest retains the released minimum dependency while this target
proves the separately reviewed release candidate.

The target is intentionally separate from CI because its evidence depends on
real Unix PTY and process-group behavior. `make help` lists the other local
verification targets.

Application code should construct commands with `CommandBuilder`; the test
fixture invokes `/bin/sh` only to create deterministic descendant processes.

## Provenance

This tree is derived from the `pty` crate in the wezterm repository. See
[UPSTREAM.md](UPSTREAM.md) and [LICENSE.md](LICENSE.md).
