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
