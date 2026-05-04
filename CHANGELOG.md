# Changelog

## Unreleased

Changed:

- the GitHub repo is now `russellromney/litelease`
- the published Rust wrapper crate is now named `litelease`
- the repo is now dual-licensed under MIT or Apache-2.0
- `packages/litelease-py` has been retired as a shipped surface
- Python usage is now documented through stdlib `sqlite3` plus the
  SQLite extension examples instead of a separate package API

### Phase 016 — distribution and public stress

Added:

- local distribution commands:
  - `make build-ext`
  - `make dist-ext`
  - `make smoke-ext`
- portable helper scripts for staging release-shaped extension assets
  and `.sha256` files
- release workflow checksum generation and upload for extension assets
- a release-built shared-library smoke test in
  `packages/litelease/tests/extension_load.rs`
- `packages/litelease/tests/public_stress.rs`, a repeated public-surface
  stress suite over wrapper and SQL boundaries

Changed:

- extension release assets now use stable platform-specific names like
  `litelease-extension-linux-x86_64.so` and ship matching `.sha256` files
- release/install docs now show the real build, stage, and smoke path
  rather than only crate-local build steps

Clarified:

- this phase widens proof and improves distribution, not lease or
  schema semantics

### Phase 001 — core lease contract

Added:

- initial repo scaffold for `litelease`
- first real `litelease-core` Rust core crate
- SQLite bootstrap for `litelease_resources`
- Rust `claim`, `renew`, `release`, and time-aware `inspect` helpers
- Phase 001 tests for claim, expiry, renew, release, and monotonic fencing behavior
- first pass of `README.md`, `ROADMAP.md`, and `SYSTEM.md`
- phase artifacts for `001-core-lease-contract` with spec, plan,
  review/decision, and commit-trace records

Clarified:

- Litelease is a single-machine lease / fencing primitive for SQLite apps, not a distributed coordination system
- Phase 001 stops at the Rust core contract and tests
- the repo phase workflow centers on `spec-diff.md`, `plan.md`, and `reviews_and_decisions.md`

### Phase 002 — first Rust wrapper

Added:

- first Rust wrapper crate in `packages/litelease`
- explicit wrapper bootstrap plus owned/borrowed wrapper types
- wrapper tests for negative bootstrap behavior, wrapper/core interop, TTL parity, and fencing-token monotonicity across wrapper/core calls

Clarified:

- the Rust wrapper stays thin
- bootstrap remains explicit
- wall clock is expiry bookkeeping, not an ordering primitive

### Phase 003 — first SQLite extension surface

Added:

- first SQLite loadable-extension crate in `litelease-extension`
- first `litelease_*` SQL surface:
  - `litelease_bootstrap()`
  - `litelease_claim(name, owner, ttl_ms, now_ms)`
  - `litelease_renew(name, owner, ttl_ms, now_ms)`
  - `litelease_release(name, owner, now_ms)`
  - `litelease_owner(name, now_ms)`
  - `litelease_token(name)`
- direct SQL-function tests in `litelease-core`
- SQL/Rust interop tests in `packages/litelease`

Clarified:

- the SQL surface is real, keeps `now_ms` explicit, and shares semantics with the Rust core rather than reimplementing lease logic

### Phase 004 — transactional SQL mutators

Added:

- transaction-aware internal `claim_in_tx`, `renew_in_tx`, and `release_in_tx` helpers in `litelease-core`
- explicit-transaction SQL tests for commit and rollback behavior
- multi-mutator transaction tests for commit and rollback behavior
- read-helper-in-transaction proof
- semantic-stress SQL test inside an explicit transaction
- savepoint rollback test for the SQL surface
- dedicated deferred multi-connection contention test that pins a lock/busy failure in the in-transaction SQL path

Changed:

- `litelease_claim`, `litelease_renew`, and `litelease_release` now participate in caller-owned explicit transactions and savepoints instead of failing with SQLite's nested-transaction error
- the autocommit SQL path still preserves the direct Rust path's `BEGIN IMMEDIATE` behavior
- the baseline docs now reflect the transactional SQL contract
- the baseline docs now state plainly that fencing safety beyond SQLite requires downstream consumers to carry and compare Litelease's token

### Phase 005 — borrowed Rust transaction contract

Added:

- public `claim_in_tx`, `renew_in_tx`, and `release_in_tx` helpers in
  `litelease-core`
- `Error::NotInTransaction` fail-fast guard for public in-transaction
  Rust helpers
- borrowed-wrapper tests for explicit transaction commit/rollback,
  multi-mutator commit/rollback, savepoint participation, and borrowed
  semantic-stress behavior
- crate-level docs in `litelease-core` that explain when to use the
  transaction-owning helpers versus the caller-owned `*_in_tx` helpers

Changed:

- `LiteleaseRef::claim`, `renew`, and `release` now mirror the SQL
  extension's transaction behavior: autocommit opens its own
  `BEGIN IMMEDIATE`, while an already-open transaction or savepoint is
  reused instead of triggering a nested-transaction failure
- `LiteleaseRef` now uses `borrowed()` instead of the old `as_ref()`
  method name

### Phase 006 — Rust transaction handle

Added:

- sanctioned wrapper-owned `Litelease::transaction()` path in
  `packages/litelease`
- `Transaction<'db>` handle with `inspect`, `claim`, `renew`,
  `release`, `conn()`, `commit()`, and `rollback()`
- wrapper tests for transaction-handle commit/rollback,
  multi-mutator commit/rollback, drop-rollback, direct `inspect`,
  direct `renew`, and semantic parity
- wrapper README example for combining a business write and a lease
  mutation in one `BEGIN IMMEDIATE` transaction boundary

Changed:

- `Litelease::transaction()` now takes `&mut self` and uses the checked
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
- recommended-default documentation for `Litelease`,
  `Litelease::transaction()`, and `LiteleaseRef`

Changed:

- `packages/litelease/src/lib.rs` is split so the public wrapper surface
  stays small and tests live in dedicated modules
- wrapper semantic-stress tests now use deterministic explicit-time
  core helpers instead of sleeping for expiry

### Phase 008 — core crate rename

Changed:

- renamed the Rust core crate and directory from `litelease-core` to
  `litelease-core`
- updated current docs, dependency declarations, and imports so Honker
  can eventually depend on Litelease without the dependency direction
  reading backwards

### Phase 009 — Python binding

Added:

- local-development Python package in `packages/litelease-py`
- PyO3 native module exposed as `litelease._litelease_native`
- pure-Python dataclass result shapes for `LeaseInfo`, `ClaimResult`,
  `RenewResult`, and `ReleaseResult`
- Python `Litelease` wrapper with explicit `bootstrap()`, `inspect`,
  `claim`, `renew`, and `release`
- Python transaction context manager for atomic business writes plus
  lease mutations
- root `Makefile` and Python dev tooling with pinned Rust, extension,
  Python build, and Python test commands
- Python tests for lifecycle behavior, transaction commit/rollback,
  context-manager state, parameter binding, error mapping, and SQL
  extension interop on one database file
- Rust integration test coverage for the built `litelease-extension`
  cdylib artifact and every registered `litelease_*` SQL function

Clarified:

- Phase 009 proves local development install and test shape only; PyPI
  publishing remains out of scope
- the Python binding is binding-owned and does not yet wrap caller-owned
  `sqlite3.Connection` transactions
- Python `tx.execute` is a single-statement helper and raises
  `LiteleaseError` for SQL syntax errors or multi-statement input

### Phase 010 — Python binding hardening

Changed:

- `Litelease.transaction()` no longer eagerly opens `BEGIN IMMEDIATE`;
  the transaction opens inside `Transaction.__enter__`
- `Transaction` is single-use and context-manager-first; `__enter__`
  raises `LiteleaseError` if the transaction is already entered or
  finished, and pre-`__enter__` verb calls raise with a message
  pointing at `with db.transaction() as tx:`
- if `begin_transaction` fails inside `__enter__`, the `Transaction`
  remains unentered so the same instance can be re-entered after the
  contention clears
- `Transaction.__del__` is removed; the native
  `Drop for NativeLitelease` is the only remaining transaction safety
  net
- `Transaction` is no longer exported from `litelease.__all__`; users
  reach it only through `db.transaction()`
- `packages/litelease-py/Cargo.toml` aligns to Rust edition `2021`,
  matching `litelease-core` and `litelease-extension`

Added:

- direct Python tests for `tx.inspect`, `tx.renew`, and `tx.release`
  inside an active transaction
- Python tests pinning the entered / unentered / single-use contract
  on `Transaction` and proving that an unentered `Transaction` does
  not hold a SQLite write lock
- `LiteleaseError` covers non-lease native errors (a SQL syntax error
  in `tx.execute` raises `LiteleaseError`)
- a regression test pinning the rusqlite multi-statement reject
  behavior of `tx.execute`
- `packages/litelease-py/README.md` section directing callers who own
  a stdlib `sqlite3.Connection` at the SQL extension path, with a
  working `enable_load_extension` snippet
- root `README.md` "Choosing a surface" section showing SQL
  extension, Python binding, and Rust wrapper side by side

### Phase 011 — deterministic invariant runner

Added:

- `litelease-core/tests/invariants.rs`, a deterministic core-level
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

- `litelease-core/tests/sqlite_matrix.rs`, a file-backed SQLite behavior
  matrix for direct core calls and the in-process SQL extension surface
- `packages/litelease/tests/sqlite_matrix.rs`, a wrapper-only matrix for
  `Litelease::transaction()`, `LiteleaseRef`, typed `Savepoint`, and a WAL
  autocommit lease-busy row
- fresh-tempdir-per-row coverage for autocommit, deferred `BEGIN`,
  `BEGIN IMMEDIATE`, savepoints, two connections, `busy_timeout = 0`,
  one small nonzero `busy_timeout`, and `journal_mode = WAL` versus
  `DELETE`
- explicit deferred-contention rows for `claim_in_tx`, `renew_in_tx`,
  and `release_in_tx`

Changed:

- `litelease-core::attach_litelease_functions` now preserves underlying
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

### Phase 013 — schema and data-integrity hardening

Added:

- strict bootstrap-time schema validation in `litelease-core` for
  preexisting `litelease_resources` tables
- public `Error::SchemaMismatch { reason: String }` for incompatible
  persisted schema drift
- `litelease-core/tests/integrity.rs`, a file-backed hardening suite for
  schema drift, invalid persisted rows, token near-overflow, TTL edges,
  bootstrap idempotency, existing-live-lease preservation, and unusual
  text round-trip behavior

Changed:

- `bootstrap_litelease_schema(conn)` no longer silently accepts arbitrary
  preexisting `litelease_resources` table shapes; it now fails loudly when
  the persisted schema drifts from the proved contract
- schema matching is intentionally strict in this phase: exact six
  columns, column order, declared type text, required nullability,
  primary-key position on `name`, and the two load-bearing CHECK
  constraints
- invalid persisted rows that violate the owner/expiry pairing
  invariant or `token >= 1` now have direct hardening proof across
  public core operations
- dead `BOUNCER_SCHEMA_VERSION` was removed; schema version remains
  implicit in the exact proved table shape rather than stored separately

Clarified:

- this phase adds loud failure, not migration or repair; old/incompatible
  schema is treated as drift until a future upgrade story exists
- `SchemaMismatch.reason` is human-readable diagnostic text, not part of
  the stable caller contract

### Phase 014 — docs as safety rails

Added:

- `litelease-core/tests/pragma_matrix.rs`, a file-backed pragma-neutrality
  matrix for core bootstrap/mutators plus SQL registration and SQL
  function calls
- `packages/litelease/tests/pragma_matrix.rs`, a wrapper pragma-neutrality
  matrix for wrapper-owned bootstrap, borrowed mutators,
  `Litelease::transaction()`, and typed savepoints
- a compact root `README.md` safety-rails section covering lease busy
  versus SQLite busy/locked, `BEGIN IMMEDIATE`, pragma ownership,
  fencing-token obligations, and strict bootstrap drift rejection

Clarified:

- pragma-neutrality is now directly proved for the pinned five-pragmas
  set: `journal_mode`, `synchronous`, `busy_timeout`, `locking_mode`,
  and `foreign_keys`
- Litelease does not set or normalize caller-owned pragma policy as a
  side effect of bootstrap or lease operations across the sanctioned
  core, SQL, and wrapper surfaces

### Phase 015 — user journey acceptance

Added:

- `packages/litelease/tests/user_journeys.rs`, a direct public-surface
  acceptance suite covering fresh bootstrap + first claim,
  second-caller busy, release/reclaim token increase,
  deterministic expiry/reclaim token increase, cross-surface state
  visibility, caller-owned transaction atomic visibility, and loud
  drifted-schema bootstrap failure
- Python acceptance rows in
  `packages/litelease-py/tests/test_bouncer.py` for cross-surface state
  visibility and loud drifted-schema bootstrap failure
- `packages/litelease/examples/three_surface_observer.rs`, a small
  wrapper observer used by the Python acceptance suite to directly
  prove Rust wrapper / SQL extension / Python binding interoperability
  on one database file

Clarified:

- this phase adds direct user-shaped proof, not new lease or schema
  semantics
- three-surface interoperability is now directly proved in one named
  journey instead of being inferred from pairwise integration rows
