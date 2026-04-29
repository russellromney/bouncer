# Bouncer System

Bouncer is a single-machine lease and ownership primitive for the Honker family.

## Current baseline

- The repo currently contains:
  - `README.md`
  - `ROADMAP.md`
  - `CHANGELOG.md`
  - this `SYSTEM.md`
  - a real `bouncer-core` crate
- a real `packages/bouncer` Rust wrapper crate
- `bouncer-core` installs a `bouncer_resources` table.
- `bouncer-core` exposes Rust helpers for `inspect`, `claim`, `renew`, and `release`.
- A resource row persists after its first successful claim so the fencing token can stay monotonic across expiry, release, and re-claim.
- `inspect(name, now_ms)` answers whether there is a live lease right now; expired or released rows do not count as owned.
- `renew` succeeds only for the current live owner.
- `release` succeeds only for the current live owner and clears ownership without resetting fencing state.
- The current proof includes file-backed multi-connection tests against a shared SQLite database file.
- `packages/bouncer` exposes an owned `Bouncer` wrapper and a borrowed `BouncerRef<'a>`.
- `packages/bouncer` now also exposes a sanctioned wrapper-owned
  `Transaction<'db>` handle via `Bouncer::transaction()`.
- `Transaction<'db>` exposes `savepoint()` for a sanctioned nested
  boundary on that wrapper-owned transaction.
- The wrapper requires explicit `bootstrap()` and does not silently create schema state in `open(path)`.
- Wrapper convenience methods use system time for lease expiry bookkeeping only.
- `bouncer-core` now exposes both transaction-owning Rust helpers
  (`claim`, `renew`, `release`) and caller-owned transaction helpers
  (`claim_in_tx`, `renew_in_tx`, `release_in_tx`).
- The public `*_in_tx` helpers fail fast with `Error::NotInTransaction`
  when called on an autocommit connection.
- Stale-actor safety still flows through SQLite writer serialization and fencing tokens, not through wall-clock ordering.
- Bouncer can only provide the fencing token. Downstream callers must include and compare that token at their external side-effect boundary if they want stale-actor protection beyond SQLite itself.
- The wrapper stays pragma-neutral; callers own connection policy such as `journal_mode` and `busy_timeout`.
- Wrapper tests prove negative bootstrap behavior and wrapper/core interoperability on the same database file.
- `BouncerRef` mutators now mirror the SQL extension's transaction
  model: in autocommit mode they open their own `BEGIN IMMEDIATE`
  through the core helpers, and inside an existing transaction or
  savepoint they reuse the caller's current atomic boundary.
- `Bouncer::transaction()` now uses Rust borrow exclusivity to prevent
  same-wrapper autocommit calls or a second wrapper-owned transaction
  from overlapping the open transaction on that connection.
- The wrapper transaction handle exposes `inspect`, `claim`, `renew`,
  `release`, `conn()`, `commit()`, and `rollback()`.
- The wrapper savepoint handle exposes `inspect`, `claim`, `renew`,
  `release`, `conn()`, `commit()`, and terminal consuming
  `rollback()`.
- The recommended wrapper default is `Bouncer` for simple autocommit
  lease operations, `Bouncer::transaction()` when business writes and
  lease mutations must commit or roll back together, and `BouncerRef`
  when the caller owns the SQLite connection or current
  transaction/savepoint state.
- Wrapper tests now also prove borrowed-path commit/rollback,
  multi-mutator transactions, semantic-stress behavior, and savepoint
  participation on the same database file.
- Wrapper tests now also prove transaction-handle commit/rollback,
  direct `inspect` and `renew`, drop-rollback, and semantic parity.
- Wrapper tests now prove transaction and savepoint durability from a
  fresh connection, savepoint renew/release coverage, savepoint commit
  plus outer rollback behavior, and deterministic explicit-time
  semantic stress instead of sleep-based expiry waits.
- `packages/bouncer/src/lib.rs` stays focused on the public wrapper
  surface; wrapper tests live in split test modules.
- Contention semantics are still primarily proven at the core layer; the wrapper proves thin delegation, interop, and borrowed transaction participation rather than a new concurrency model.
- A local-development Python package now exists in
  `packages/bouncer-py`.
- The Python package imports as `bouncer` and uses a PyO3 native module
  at `bouncer._bouncer_native`.
- The Python binding calls `bouncer-core` directly for lease semantics
  and keeps result objects as pure-Python dataclasses.
- The Python binding exposes explicit `bootstrap()`, `inspect`,
  `claim`, `renew`, and `release`.
- The Python binding exposes a `with db.transaction() as tx:` context
  manager for business writes plus lease mutations in one
  `BEGIN IMMEDIATE` boundary.
- The supported Python V1 transaction shape is the `with` block; direct
  non-context-manager transaction use is not part of the documented
  contract.
- `Bouncer.transaction()` no longer eagerly opens `BEGIN IMMEDIATE`.
  The transaction opens inside `Transaction.__enter__` and `Transaction`
  is single-use; entering twice or entering after explicit
  `commit`/`rollback` raises `BouncerError`. Pre-`__enter__` verb
  calls raise `BouncerError` with a message pointing at
  `with db.transaction() as tx:`.
- The Python `Transaction` has no `__del__` safety net; native handle
  teardown is the only safety path for orphaned
  `transaction_active = True`.
- While a Python transaction is active, top-level lease operations on
  that handle fail loudly; callers use the transaction handle until it
  commits or rolls back.
- Python tests prove explicit bootstrap, full lifecycle behavior,
  transaction commit/rollback coupling, terminal context-manager
  behavior, parameter binding, and SQLite-extension interop on one
  database file.
- Rust tests now also build and load the `bouncer-extension` cdylib
  artifact through rusqlite and exercise every `bouncer_*` function.
- a real `bouncer-extension` loadable-extension crate exists in the workspace.
- `bouncer-core` now also owns the first `bouncer_*` SQL function registration surface via `attach_bouncer_functions`.
- The current SQL surface is:
  - `bouncer_bootstrap()`
  - `bouncer_claim(name, owner, ttl_ms, now_ms)`
  - `bouncer_renew(name, owner, ttl_ms, now_ms)`
  - `bouncer_release(name, owner, now_ms)`
  - `bouncer_owner(name, now_ms)`
  - `bouncer_token(name)`
- SQL and Rust interoperate against the same database file and share the same lease semantics and fencing state.
- The SQL surface keeps time explicit. It does not read `now()` from inside SQLite.
- SQL mutators now work both in autocommit mode and inside an already-open explicit transaction or savepoint on the caller's connection.
- In autocommit mode, mutating SQL helpers preserve the direct Rust path's `BEGIN IMMEDIATE` behavior.
- Inside a caller-owned transaction or savepoint, mutating SQL helpers reuse the current transaction state rather than opening a nested transaction.
- In that in-transaction path, lock-upgrade timing follows the caller's outer transaction mode rather than forcing a new `BEGIN IMMEDIATE`.
- Core tests now prove commit/rollback behavior, multi-mutator transactions, read helpers inside a transaction, semantic-stress behavior inside a transaction, a savepoint rollback path, and a lock/busy failure under deferred multi-connection writer contention.

## Current intent

- Bouncer answers "who owns this named resource right now?" for normal SQLite apps.
- Bouncer is for the single-machine SQLite stack, not distributed coordination.
- Bouncer should stay small, inspectable, and boring.

## Boundaries that already matter

- `SYSTEM.md` should describe only the current proved baseline, not the desired finished system.
- Future semantic changes should be proposed through new `.intent/phases/...` artifacts before the code drifts.
- Honker remains the generic async substrate for the family.

## Non-goals

- This repo is not distributed consensus.
- This repo is not a workflow engine.
- This repo does not yet publish language bindings to package registries.
