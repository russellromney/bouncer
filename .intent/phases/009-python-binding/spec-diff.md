What changes:

- Bouncer gets its first non-Rust binding: a Python package that proves
  the current core shape is usable from another language.
- The Python package should be a thin wrapper over the existing
  Bouncer-owned SQLite semantics, not a second lease implementation.
- The package should expose the same first-use story as the Rust
  wrapper:
  - open a database path
  - explicit `bootstrap()`
  - `inspect`
  - `claim`
  - `renew`
  - `release`
- The package should expose a sanctioned transaction path so Python
  callers can commit or roll back business writes and lease mutations
  together.
- The binding should return small Python-shaped result objects rather
  than leaking Rust enum formatting or raw SQLite rows.
- The binding should include Python tests that prove interop with the
  Rust/SQLite contract on one database file.
- The binding should call `bouncer-core` directly for lease
  semantics. It may duplicate binding-edge policy such as system-time
  reads and runtime transaction guards, but it must not duplicate the
  lease state machine.
- Python transaction SQL execution should bind positional parameters
  through rusqlite. It must not interpolate user values into SQL.
- The Python transaction is an explicit context manager. Entering
  `with db.transaction() as tx:` commits on clean block exit and
  rolls back on exception. Calling `tx.commit()` or `tx.rollback()`
  inside the block marks the transaction finished, after which any
  further `tx.*` operation and any context-manager exit action are
  no-ops or raise `bouncer.BouncerError`.
- While a transaction is active on a `bouncer.Bouncer` handle, all
  top-level `db.claim`, `db.renew`, `db.release`, and `db.inspect`
  calls on that handle raise `bouncer.BouncerError`. Callers use
  the `tx` object for the entire transaction. This mirrors the Rust
  wrapper's compile-time exclusivity guarantee at runtime.
- Native binding types are non-`Sync`. The Python `Bouncer` handle
  is single-threaded; cross-thread use requires opening a new
  handle.
- The Python result objects are pure-Python dataclasses with a flat
  shape:
  - `LeaseInfo(name, owner, token, lease_expires_at_ms)`
  - `ClaimResult(acquired, lease, current)`
  - `RenewResult(renewed, lease, current)`
  - `ReleaseResult(released, name, token, current)`

What does not change:

- Phase 009 does not change core lease semantics.
- Phase 009 does not add nested savepoints.
- Phase 009 does not add broad framework integrations.
- Phase 009 does not publish to PyPI. It proves local development
  install and test shape only.
- Phase 009 does not promise caller-owned `sqlite3.Connection`
  integration. Callers that own a Python `sqlite3.Connection` can use
  the SQL extension path later; this phase proves the binding-owned
  path first.
- Phase 009 does not add the deterministic simulation harness.

How we will verify it:

- Python tests can open a file, bootstrap explicitly, and run the full
  claim / busy / inspect / renew / release cycle.
- A Python claim is visible from Rust core inspection on a fresh
  connection, or an equivalent cross-surface test proves the same file
  contract.
- A Rust or SQL-created lease is visible from the Python binding.
- Python transaction tests prove business writes and lease mutations
  commit together and roll back together.
- Python tests load `bouncer-extension` into stdlib `sqlite3` for at
  least one cross-surface verification path.
- The binding tests avoid sleep-based expiry checks unless there is no
  practical alternative; expiry semantics remain pinned in
  `bouncer-core`.
- `cargo test -p bouncer -p bouncer-core`, the pinned Python test
  command, and formatting/lint checks pass.
