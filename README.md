# sysprims-pty

Cross-platform **owned PTY sessions** with race-free process containment,
built on [sysprims](https://github.com/3leaps/sysprims).

Use this crate when you need a real native PTY or ConPTY, opaque byte I/O and
resize, and an honest final outcome for the exact child and its descendants
when the tab, test, or agent closes.

It is derived from [`portable-pty`](https://crates.io/crates/portable-pty)
0.9.0, so familiar command / pair / master / slave / I/O / resize types remain
available as a **migration lane**. Compatibility is the on-ramp, not the
product: this crate does not promise identity with upstream `portable-pty`,
and it adds what portable-pty leaves to you — race-free contained spawn and
an owned, verifiable process lifecycle.

```toml
# Cargo.toml
sysprims-pty = "0.9.0"
```

The library target is still named `portable_pty`, so existing imports can
keep working:

```toml
portable_pty = { package = "sysprims-pty", version = "0.9.0" }
```

## What you can build

One launch, one owned session, one honest final outcome. Use it for an agent
console, IDE terminal panel, interactive test harness, REPL supervisor, or
task runner that:

1. asks whether contained spawn is supported before starting,
2. starts the session in a single spawn-time transaction,
3. talks to the PTY with opaque byte I/O and resize,
4. closes explicitly or lets the leader complete naturally,
5. reads containment, completion, and reap evidence with no process-group,
   Job, or wait glue of your own.

On Unix today, `SlavePty::spawn_contained_command` installs the prepared
sysprims acquisition hook in the PTY-owned spawn, validates its sealed
same-spawn receipt, and returns an owned `ContainmentGuard<ContainedPtyChild>`.

## What this is not

- a terminal emulator (no VT parsing, scrollback, or rendering)
- a multiplexer or collaboration layer
- a shell policy / authorization product
- a general process supervisor or generic spawner
- a drop-in replacement that matches every `portable-pty` API and bug
- an OS non-escape sandbox

Generic process identity, Job/group evidence, and receipts live in
`sysprims`. This crate composes those primitives around the PTY spawn seam.

## Containment semantics

- `TreeKillReliability::Guaranteed` means race-free session/group
  acquisition and group-signaling eligibility.
- The guard exclusively owns child observation and reap through a terminal
  lifecycle transition.
- Completion evidence is reported independently as `Empty`, `Survivors`,
  or `Unknown`.
- A cooperative Unix descendant can still leave its acquired group.
  Guaranteed acquisition is not an OS-enforced non-escape guarantee.
- Unsupported implementations, including Windows in this tree, reject the
  guarded API before spawning.

## Owned real-PTY examples

The integration tests contain two small, executable public-API examples. Each
opens a real controlling PTY, starts at least one descendant, and proves
`Guaranteed` acquisition, supported `Empty` completion, and exact leader reap:

1. [`owned_empty_explicit_close_with_descendant`](tests/contained_spawn.rs)
   calls `ContainmentGuard::terminate`.
2. [`owned_empty_natural_leader_exit_with_descendant`](tests/contained_spawn.rs)
   polls `ContainmentGuard::try_complete`, retrying only `Ok(None)` while the
   leader is running. Any error fails the scene.

```console
make test-owned-pty-empty
```

The runner uses the sibling `../sysprims` checkout by default, verifies a
clean exact revision, then applies it as a local source override in an
isolated copy. Set `SYSPRIMS_ROOT` to use another checkout. The package
manifest retains the released minimum dependency; this target proves a
separately reviewed candidate.

The target is intentionally separate from ordinary CI because its evidence
depends on real Unix PTY and process-group behavior. `make help` lists the
other local verification targets.

Application code should construct commands with `CommandBuilder`. The test
fixture invokes `/bin/sh` only to create deterministic descendant processes.

## Provenance

This tree is derived from the `pty` crate in the wezterm repository. See
[UPSTREAM.md](UPSTREAM.md) and [LICENSE.md](LICENSE.md).
