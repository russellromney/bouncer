What changes:

- The Python binding becomes context-manager-first. `Bouncer.transaction()`
  no longer eagerly opens `BEGIN IMMEDIATE`; it returns a `Transaction`
  in an unentered state. The transaction opens inside
  `Transaction.__enter__` and finishes inside `Transaction.__exit__`,
  `tx.commit()`, or `tx.rollback()`.
- Calling any `tx.*` method on a `Transaction` that has not been entered
  raises `bouncer.BouncerError` with a message that points the caller
  at `with db.transaction() as tx:`.
- A `Transaction` is single-use. `__enter__` raises `BouncerError` if
  the `Transaction` has already been entered or already finished.
  Reopening a transaction requires a fresh `db.transaction()` call.
- If `begin_transaction` fails inside `__enter__` (for example, because
  another connection holds a write lock), the `Transaction`'s
  `_entered` flag remains `False` so the same instance can be re-entered
  once the contention clears. The error propagates as `BouncerError`.
- `Transaction.__del__` is removed. After the context-manager-first
  change, the Python object holds no native transaction state until
  `__enter__` runs, and the native `Drop for NativeBouncer` already
  rolls back any orphaned `transaction_active = True` on handle teardown.
  Documented behavior change: a user who manually calls `tx.__enter__()`
  outside a `with` block and GCs the `Transaction` without
  `commit()` or `rollback()` leaks `transaction_active = True` on the
  native handle until the underlying `Bouncer` is also dropped. The
  next `db.transaction()` on that `Bouncer` raises `BouncerError`.
  Honest fail-loud, not silent fix.
- `Transaction` is removed from `bouncer.__all__` and from the public
  re-export in `bouncer/__init__.py`. Users reach `Transaction` only
  through `db.transaction()`. The class name stays `Transaction`.
- `packages/bouncer-py/Cargo.toml` aligns its Rust edition to `2021`,
  matching `bouncer-core` and `bouncer-extension`. Broader Honker-family
  alignment to `2024` is a separate phase.
- The Python package README adds a short section telling users who
  already own a stdlib `sqlite3.Connection` to use the SQL extension
  path instead. Includes a five-line working code example
  (`sqlite3.connect → enable_load_extension → load_extension →
  SELECT bouncer_bootstrap()`).
- The Python package README adds a one-line note that `tx.execute`
  runs a single SQL statement; multi-statement strings have the
  trailing statements silently dropped (rusqlite/SQLite behavior).
- The root `README.md` adds one short example block that shows the
  three caller surfaces side by side: SQL extension for SQL-only
  callers, Python binding for typed Python callers, Rust wrapper for
  Rust callers.
- Three previously-untested Python in-transaction verbs gain direct
  tests: `tx.inspect` returning a live lease, `tx.renew` extending
  and rejecting wrong-owner, and `tx.release` clearing the owner.
- A new test pins that `BouncerError` covers non-lease native errors:
  a SQL syntax error in `tx.execute` raises `BouncerError`.
- A new regression test pins the current `tx.execute` single-statement
  silent-drop behavior so future changes notice if the contract
  shifts.

What does not change:

- Lease semantics, fencing token monotonicity, expiry rules, schema.
- Result-type shapes (`LeaseInfo`, `ClaimResult`, `RenewResult`,
  `ReleaseResult`) on either the Python or Rust side.
- The SQL extension surface or its function names.
- The Rust wrapper's public behavior.
- The pinned `make` targets, `uv` dev-tool dependency, or
  `bouncer-py`'s own `[workspace]` isolation.
- `bouncer-core` is still the binding link target. The Python binding
  does not start wrapping `packages/bouncer` or routing through the
  SQL extension.

How we will verify it:

- Existing 11 Python tests continue to pass after the move of
  `BEGIN IMMEDIATE` into `Transaction.__enter__`.
- A new test asserts that calling `tx.claim` on a `Transaction`
  before `__enter__` raises `bouncer.BouncerError`.
- A new test asserts that constructing a `Transaction` without
  entering it does not open a SQLite transaction on the underlying
  connection (top-level `db.claim` still works).
- `cargo build --manifest-path packages/bouncer-py/Cargo.toml`
  succeeds with the new edition.
- `make test` passes.
- The README diffs render and stay consistent with the existing
  package README example.
