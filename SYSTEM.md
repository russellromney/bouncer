# Bouncer System

Bouncer is a single-machine SQLite lease and ownership primitive for
SQLite apps. It answers one question: who owns this named resource right
now? It is not consensus, not a workflow engine, and not a distributed
coordinator. It is a small SQLite state machine with fencing tokens.

`SYSTEM.md` describes the current proved baseline. Future semantic changes
should be recorded in phase artifacts before the code drifts.

## Core Contract

`bouncer-core` owns the schema and lease semantics. It installs a
`bouncer_resources` table and exposes `inspect`, `claim`, `renew`, and
`release`.

A resource row persists after the first successful claim. Expiry and release
clear live ownership, but they do not reset fencing state. The next successful
claim receives a larger fencing token, so tokens remain monotonic across
claim, expiry, release, and reclaim.

`inspect(name, now_ms)` returns a live lease only. Expired or released rows do
not count as owned. `renew` succeeds only for the current live owner and
never shortens a live lease; expiry becomes
`max(current_expiry, now_ms + ttl_ms)`. `release` succeeds only for the
current live owner.

The core keeps time explicit through `now_ms`. Time is used for lease expiry
bookkeeping, not as an ordering primitive. Stale-actor safety flows through
SQLite writer serialization plus fencing tokens. Bouncer can provide the token;
downstream systems must carry and compare it at their external side-effect
boundary if they want stale-actor protection outside SQLite.

Core mutators come in two shapes. `claim`, `renew`, and `release` own their
transaction and open `BEGIN IMMEDIATE`. `claim_in_tx`, `renew_in_tx`, and
`release_in_tx` reuse a caller-owned transaction or savepoint and fail fast with
`Error::NotInTransaction` when called on an autocommit connection.

`bootstrap_bouncer_schema(conn)` now has a strict persisted-schema contract.
On a fresh database it creates `bouncer_resources`. On an existing database it
validates the current table shape and fails loudly with
`Error::SchemaMismatch { reason }` if the table has drifted from the proved
schema. The current contract is exact six-column shape, column order, declared
type text, required nullability, primary-key position on `name`, and the two
load-bearing invariants:

- `token >= 1`
- `(owner IS NULL AND lease_expires_at_ms IS NULL) OR (owner IS NOT NULL AND lease_expires_at_ms IS NOT NULL)`

This is not a migration engine and not a structural-superset policy. Extra
columns, affinity-compatible-but-different declared types, or weakened
constraints are drift in the current baseline.

Invalid persisted rows also fail loudly through the core API. If a loaded row
violates the owner/expiry pairing invariant or has `token <= 0`, the core
returns `Error::InvalidLeaseRow(...)` instead of partially operating on it.

## SQL Extension

`bouncer-extension` is a SQLite loadable extension. The registration code lives
in `bouncer-core::attach_bouncer_functions`, so SQL callers and Rust callers
share the same lease state machine.

The SQL surface is:

- `bouncer_bootstrap()`
- `bouncer_claim(name, owner, ttl_ms, now_ms)`
- `bouncer_renew(name, owner, ttl_ms, now_ms)`
- `bouncer_release(name, owner, now_ms)`
- `bouncer_owner(name, now_ms)`
- `bouncer_token(name)`

The SQL surface keeps time explicit and does not read SQLite's current time.
Mutating SQL functions work in autocommit mode and inside an already-open
transaction or savepoint. In autocommit mode they preserve the core
`BEGIN IMMEDIATE` behavior. Inside caller-owned transaction state, they reuse
that boundary, so lock-upgrade timing follows the caller's outer transaction
mode.

When a `bouncer_*` SQL function hits an underlying SQLite lock failure,
SQLite/rusqlite can collapse some UDF callback lock failures to a generic
`SQLITE_ERROR` while preserving only busy/locked text. Outside that known
scalar-function boundary quirk, Bouncer preserves the native SQLite
`BUSY` / `LOCKED` error rather than turning it into a separate lease
semantic.

## Rust Wrapper

`packages/bouncer` is the Rust convenience wrapper. It exposes an owned
`Bouncer`, a borrowed `BouncerRef<'a>`, a wrapper-owned `Transaction<'db>`, and
a `Savepoint<'db>`.

The recommended default is `Bouncer` for simple autocommit lease operations.
Use `Bouncer::transaction()` when business writes and lease mutations must
commit or roll back together. Use `BouncerRef` when the caller already owns the
SQLite connection or current transaction/savepoint state.

Wrapper convenience methods read system time for lease expiry bookkeeping only.
`BouncerRef` mutators mirror the SQL extension transaction model: in
autocommit mode they open their own `BEGIN IMMEDIATE` through the core helpers;
inside an existing transaction or savepoint they reuse the caller's current
atomic boundary.

`Bouncer::transaction()` uses Rust borrow exclusivity to prevent same-wrapper
autocommit calls or a second wrapper-owned transaction from overlapping the
open transaction on that connection. The transaction handle exposes `inspect`,
`claim`, `renew`, `release`, `conn()`, `commit()`, `rollback()`, and
`savepoint()`.

`Savepoint<'db>` exposes `inspect`, `claim`, `renew`, `release`, `conn()`,
`commit()`, and a terminal consuming `rollback()`.

The wrapper requires explicit `bootstrap()` and does not create schema state in
`open(path)`. It is also pragma-neutral: callers own connection policy such as
`journal_mode`, `synchronous`, `busy_timeout`, `locking_mode`, and
`foreign_keys`.

## Python Binding

`packages/bouncer-py` imports as `bouncer` and uses a PyO3 native module at
`bouncer._bouncer_native`. The binding calls `bouncer-core` directly for lease
semantics and returns pure-Python dataclasses for result objects. It is a thin
owned-connection convenience layer, not the base interoperability surface for
Python. The SQLite extension remains the caller-owned connection path.

The public Python package exposes `Bouncer`, `BouncerError`, result dataclasses,
and `open`. It exposes `bootstrap`, `inspect`, `claim`, `renew`, and `release`
on `Bouncer`. `Transaction` is reached through `db.transaction()` and is not
exported from `bouncer.__all__`.

The Python binding owns its SQLite connection. Python callers who already own a
`sqlite3.Connection` should use the SQL extension surface instead.

The supported Python V1 transaction shape is `with db.transaction() as tx:`.
`Bouncer.transaction()` is side-effect-free and returns an unentered
transaction handle. `BEGIN IMMEDIATE` opens inside `Transaction.__enter__`.
`Transaction` is single-use: entering twice or entering after explicit
`commit`/`rollback` raises `BouncerError`. Calling transaction verbs before
`__enter__` raises `BouncerError` with a message pointing at
`with db.transaction() as tx:`.

If `__enter__` fails while beginning the transaction, the transaction remains
unentered and can be entered again after the contention clears. While a Python
transaction is active, top-level lease operations on the same `Bouncer` handle
fail loudly; callers must use the transaction handle until it commits or rolls
back. The Python `Transaction` has no `__del__` safety net; native handle
teardown is the only orphan safety path for `transaction_active = True`.

`tx.execute(sql, params=None)` binds positional parameters and returns the
affected-row count. It is a single-statement helper. SQL syntax errors and
multi-statement strings raise `BouncerError`.

## Proof Baseline

The core tests prove lease semantics, file-backed multi-connection behavior,
transaction commit/rollback, multi-mutator transactions, read helpers inside a
transaction, semantic stress inside a transaction, savepoint rollback, and
lock/busy behavior under deferred multi-connection writer contention.

The core also now has a deterministic invariant runner over seeded
explicit-time autocommit operation sequences. It proves high-level lease
invariants such as one live owner per sampled time, monotonic fencing
tokens, post-release row shape, non-mutating busy/wrong-owner paths, and
renew preserving or extending expiry without shortening it.

The core and SQL-extension tests also now include a file-backed SQLite
behavior matrix. It proves the caller-visible split between lease busy and
SQLite lock-class failure across autocommit, deferred `BEGIN`,
`BEGIN IMMEDIATE`, savepoints, two connections, `busy_timeout = 0`, one small
nonzero `busy_timeout`, and `journal_mode = WAL` versus `DELETE`. The matrix
also pins that `claim_in_tx`, `renew_in_tx`, and `release_in_tx` leave lease
state unchanged when deferred lock upgrade fails before the mutation can land.

The core also now has a file-backed integrity hardening suite. It proves:

- strict bootstrap rejection for drifted `bouncer_resources` schema
- bootstrap idempotency on a valid current-shape schema
- bootstrap preserving an existing live lease on a valid schema
- loud failure on invalid persisted rows
- non-mutating token-overflow and TTL-overflow paths
- opaque text round-trip for unusual names and owners

Rust wrapper tests prove negative bootstrap behavior, wrapper/core interop,
SQL/Rust interop, borrowed-path commit/rollback, multi-mutator transactions,
semantic stress, transaction-handle behavior, savepoint participation,
transaction/savepoint durability from a fresh connection, savepoint
renew/release, savepoint commit plus outer rollback, and deterministic
explicit-time semantic stress.

Wrapper matrix rows now prove the sanctioned wrapper boundaries follow that
same SQLite story rather than inventing one: `Bouncer::transaction()` claims
writer intent up front, `BouncerRef` mirrors deferred lock-upgrade behavior,
typed `Savepoint` commit participates in the outer transaction boundary, and
wrapper autocommit lease-busy behavior stays stable under `journal_mode = WAL`.

The core and wrapper also now have file-backed pragma-neutrality matrices.
They prove Bouncer does not rewrite the pinned caller-owned pragma set
(`journal_mode`, `synchronous`, `busy_timeout`, `locking_mode`,
`foreign_keys`) across core bootstrap/mutators, SQL function registration and
calls, wrapper bootstrap, borrowed-path mutators, wrapper-owned transaction
mutators, and typed savepoints.

Rust tests also build and load the `bouncer-extension` cdylib through rusqlite
and exercise every `bouncer_*` function.

Python tests prove explicit bootstrap, full lifecycle behavior, transaction
commit/rollback coupling, terminal context-manager behavior, parameter binding,
`BouncerError` coverage for core and SQL errors, multi-statement rejection,
begin-failure re-entry, and SQLite-extension interop on one database file.

There is also now a small public-surface acceptance layer. The Rust and Python
acceptance tests prove fresh bootstrap plus first claim, independent second
caller busy, release/reclaim token increase, deterministic expiry/reclaim token
increase through explicit-time SQL, atomic commit visibility for business write
plus lease mutation, loud drifted-schema bootstrap failure through wrapper /
Python / SQL bootstrap, and one direct three-surface journey where Python, SQL,
and the Rust wrapper observe the same lease state transitions on one database
file. Python's role in that proof is mainly convenience and cross-surface
verification rather than defining the main product boundary.

Contention semantics are primarily proven at the core and extension boundaries.
The wrappers prove thin delegation, interop, and transaction participation
rather than a separate concurrency model.

## Boundaries

Bouncer is the lease/coordination primitive. It is intentionally much
smaller than a queue, scheduler, or workflow system.

Language bindings are typed wrappers over `bouncer-core` where rich typed
results matter. The SQLite extension remains the caller-owned connection
surface. Package registry publishing is not part of the current baseline.
