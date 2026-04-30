# Changelog

## Unreleased

### Phase 001 — core lease contract

Added:

- initial repo scaffold for `bouncer`
- first real `bouncer-honker` Rust core crate
- SQLite bootstrap for `bouncer_resources`
- Rust `claim`, `renew`, `release`, and time-aware `inspect` helpers
- Phase 001 tests for claim, expiry, renew, release, and monotonic fencing behavior
- first pass of `README.md`, `ROADMAP.md`, and `SYSTEM.md`
- `.intent/phases/001-core-lease-contract/` with spec, plan, review/decision, and commit-trace artifacts

Clarified:

- Bouncer is a single-machine lease / fencing primitive for SQLite apps, not a distributed coordination system
- Phase 001 stops at the Rust core contract and tests
- the repo phase workflow centers on `spec-diff.md`, `plan.md`, and `reviews_and_decisions.md`

### Phase 002 — first Rust wrapper

Added:

- first Rust wrapper crate in `packages/bouncer`
- explicit wrapper bootstrap plus owned/borrowed wrapper types
- wrapper tests for negative bootstrap behavior, wrapper/core interop, TTL parity, and fencing-token monotonicity across wrapper/core calls

Clarified:

- the Rust wrapper stays thin
- bootstrap remains explicit
- wall clock is expiry bookkeeping, not an ordering primitive

### Phase 003 — first SQLite extension surface

Added:

- first SQLite loadable-extension crate in `bouncer-extension`
- first `bouncer_*` SQL surface:
  - `bouncer_bootstrap()`
  - `bouncer_claim(name, owner, ttl_ms, now_ms)`
  - `bouncer_renew(name, owner, ttl_ms, now_ms)`
  - `bouncer_release(name, owner, now_ms)`
  - `bouncer_owner(name, now_ms)`
  - `bouncer_token(name)`
- direct SQL-function tests in `bouncer-honker`
- SQL/Rust interop tests in `packages/bouncer`

Clarified:

- the SQL surface is real, keeps `now_ms` explicit, and shares semantics with the Rust core rather than reimplementing lease logic

### Phase 004 — transactional SQL mutators

Added:

- transaction-aware internal `claim_in_tx`, `renew_in_tx`, and `release_in_tx` helpers in `bouncer-honker`
- explicit-transaction SQL tests for commit and rollback behavior
- multi-mutator transaction tests for commit and rollback behavior
- read-helper-in-transaction proof
- semantic-stress SQL test inside an explicit transaction
- savepoint rollback test for the SQL surface
- dedicated deferred multi-connection contention test that pins a lock/busy failure in the in-transaction SQL path

Changed:

- `bouncer_claim`, `bouncer_renew`, and `bouncer_release` now participate in caller-owned explicit transactions and savepoints instead of failing with SQLite's nested-transaction error
- the autocommit SQL path still preserves the direct Rust path's `BEGIN IMMEDIATE` behavior
- the baseline docs now reflect the transactional SQL contract
- the baseline docs now state plainly that fencing safety beyond SQLite requires downstream consumers to carry and compare Bouncer's token

### Phase 005 — borrowed Rust transaction contract

Added:

- public `claim_in_tx`, `renew_in_tx`, and `release_in_tx` helpers in
  `bouncer-honker`
- `Error::NotInTransaction` fail-fast guard for public in-transaction
  Rust helpers
- borrowed-wrapper tests for explicit transaction commit/rollback,
  multi-mutator commit/rollback, savepoint participation, and borrowed
  semantic-stress behavior
- crate-level docs in `bouncer-honker` that explain when to use the
  transaction-owning helpers versus the caller-owned `*_in_tx` helpers

Changed:

- `BouncerRef::claim`, `renew`, and `release` now mirror the SQL
  extension's transaction behavior: autocommit opens its own
  `BEGIN IMMEDIATE`, while an already-open transaction or savepoint is
  reused instead of triggering a nested-transaction failure
- `BouncerRef` now uses `borrowed()` instead of the old `as_ref()`
  method name

### Phase 006 — Rust transaction handle

Added:

- sanctioned wrapper-owned `Bouncer::transaction()` path in
  `packages/bouncer`
- `Transaction<'db>` handle with `inspect`, `claim`, `renew`,
  `release`, `conn()`, `commit()`, and `rollback()`
- wrapper tests for transaction-handle commit/rollback,
  multi-mutator commit/rollback, drop-rollback, direct `inspect`,
  direct `renew`, and semantic parity
- wrapper README example for combining a business write and a lease
  mutation in one `BEGIN IMMEDIATE` transaction boundary

Changed:

- `Bouncer::transaction()` now takes `&mut self` and uses the checked
  `transaction_with_behavior(TransactionBehavior::Immediate)` path
  instead of `new_unchecked`
- same-wrapper autocommit calls and a second wrapper-owned transaction
  can no longer overlap the open transaction through this sanctioned
  wrapper path

### Phase 007 — core hardening

Added:

- sanctioned `Transaction::savepoint()` wrapper surface with
  `Savepoint<'db>` handle methods for `inspect`, `claim`, `renew`,
  `release`, `conn()`, `commit()`, and terminal `rollback()`
- wrapper tests for savepoint rollback, savepoint commit, direct
  savepoint `renew`, direct savepoint `release`, savepoint commit plus
  outer rollback, and savepoint durability from a fresh connection
- fresh-connection durability proof for the wrapper transaction handle
- recommended-default documentation for `Bouncer`,
  `Bouncer::transaction()`, and `BouncerRef`

Changed:

- `packages/bouncer/src/lib.rs` is split so the public wrapper surface
  stays small and tests live in dedicated modules
- wrapper semantic-stress tests now use deterministic explicit-time
  core helpers instead of sleeping for expiry

### Phase 008 — core crate rename

Changed:

- renamed the Rust core crate and directory from `bouncer-honker` to
  `bouncer-core`
- updated current docs, dependency declarations, and imports so Honker
  can eventually depend on Bouncer without the dependency direction
  reading backwards

### Phase 009 — Python binding

Added:

- local-development Python package in `packages/bouncer-py`
- PyO3 native module exposed as `bouncer._bouncer_native`
- pure-Python dataclass result shapes for `LeaseInfo`, `ClaimResult`,
  `RenewResult`, and `ReleaseResult`
- Python `Bouncer` wrapper with explicit `bootstrap()`, `inspect`,
  `claim`, `renew`, and `release`
- Python transaction context manager for atomic business writes plus
  lease mutations
- root `Makefile` and Python dev tooling with pinned Rust, extension,
  Python build, and Python test commands
- Python tests for lifecycle behavior, transaction commit/rollback,
  context-manager state, parameter binding, error mapping, and SQL
  extension interop on one database file
- Rust integration test coverage for the built `bouncer-extension`
  cdylib artifact and every registered `bouncer_*` SQL function

Clarified:

- Phase 009 proves local development install and test shape only; PyPI
  publishing remains out of scope
- the Python binding is binding-owned and does not yet wrap caller-owned
  `sqlite3.Connection` transactions
- Python `tx.execute` is a single-statement helper and raises
  `BouncerError` for SQL syntax errors or multi-statement input

### Phase 010 — Python binding hardening

Changed:

- `Bouncer.transaction()` no longer eagerly opens `BEGIN IMMEDIATE`;
  the transaction opens inside `Transaction.__enter__`
- `Transaction` is single-use and context-manager-first; `__enter__`
  raises `BouncerError` if the transaction is already entered or
  finished, and pre-`__enter__` verb calls raise with a message
  pointing at `with db.transaction() as tx:`
- if `begin_transaction` fails inside `__enter__`, the `Transaction`
  remains unentered so the same instance can be re-entered after the
  contention clears
- `Transaction.__del__` is removed; the native
  `Drop for NativeBouncer` is the only remaining transaction safety
  net
- `Transaction` is no longer exported from `bouncer.__all__`; users
  reach it only through `db.transaction()`
- `packages/bouncer-py/Cargo.toml` aligns to Rust edition `2021`,
  matching `bouncer-core` and `bouncer-extension`

Added:

- direct Python tests for `tx.inspect`, `tx.renew`, and `tx.release`
  inside an active transaction
- Python tests pinning the entered / unentered / single-use contract
  on `Transaction` and proving that an unentered `Transaction` does
  not hold a SQLite write lock
- `BouncerError` covers non-lease native errors (a SQL syntax error
  in `tx.execute` raises `BouncerError`)
- a regression test pinning the rusqlite multi-statement reject
  behavior of `tx.execute`
- `packages/bouncer-py/README.md` section directing callers who own
  a stdlib `sqlite3.Connection` at the SQL extension path, with a
  working `enable_load_extension` snippet
- root `README.md` "Choosing a surface" section showing SQL
  extension, Python binding, and Rust wrapper side by side

### Phase 011 — deterministic invariant runner

Added:

- `bouncer-core/tests/invariants.rs`, a deterministic core-level
  invariant runner over `claim`, `renew`, `release`, `inspect`,
  `owner`, and `token`
- a seeded xorshift64-style test RNG with no new property-testing
  dependency
- a readable fixed-sequence lifecycle test covering first claim, busy
  claim, wrong-owner renew, valid renew, wrong-owner release, valid
  release, reclaim after release, expiry takeover, and token
  monotonicity
- a generated invariant test over 1000 seeds × 100 steps against
  in-memory SQLite with default pragmas
- a direct core regression test proving `renew` does not shorten an
  existing live lease

Changed:

- `renew` now preserves or extends a live lease expiry instead of
  blindly replacing it with `now_ms + ttl_ms`
- the renew contract is now:
  `lease_expires_at_ms = max(current_expiry, now_ms + ttl_ms)`

Clarified:

- Phase 011 proves the explicit-time autocommit lease state machine at a
  higher level through generated deterministic sequences
- caller-owned transaction generation, SQLite contention/settings
  matrix work, and corruption/manual-row hardening remain future phases

### Phase 012 — SQLite behavior matrix

Added:

- `bouncer-core/tests/sqlite_matrix.rs`, a file-backed SQLite behavior
  matrix for direct core calls and the in-process SQL extension surface
- `packages/bouncer/tests/sqlite_matrix.rs`, a wrapper-only matrix for
  `Bouncer::transaction()`, `BouncerRef`, typed `Savepoint`, and a WAL
  autocommit lease-busy row
- fresh-tempdir-per-row coverage for autocommit, deferred `BEGIN`,
  `BEGIN IMMEDIATE`, savepoints, two connections, `busy_timeout = 0`,
  one small nonzero `busy_timeout`, and `journal_mode = WAL` versus
  `DELETE`
- explicit deferred-contention rows for `claim_in_tx`, `renew_in_tx`,
  and `release_in_tx`

Changed:

- `bouncer-core::attach_bouncer_functions` now preserves underlying
  SQLite `BUSY` / `LOCKED` errors where the scalar-function boundary
  allows it instead of eagerly string-wrapping every SQLite failure as
  `UserFunctionError`

Clarified:

- the proved contract now separates lease busy from SQLite lock-class
  failure across the main Rust/SQLite surfaces
- SQLite/rusqlite can still collapse some SQL UDF callback lock
  failures to generic `SQLITE_ERROR` carrying busy/locked text; that
  fallback is now pinned explicitly in the matrix instead of being an
  implicit string-match assumption
