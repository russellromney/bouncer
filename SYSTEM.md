# Bouncer System

Bouncer is a single-machine SQLite lease and ownership primitive for the
Honker family. It answers one question: who owns this named resource right
now? It is not consensus, not a workflow engine, and not a distributed
coordinator. It is a small SQLite state machine with fencing tokens.

`SYSTEM.md` describes the current proved baseline. Future semantic changes
belong in new `.intent/phases/...` artifacts before the code drifts.

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
`journal_mode` and `busy_timeout`.

## Python Binding

`packages/bouncer-py` imports as `bouncer` and uses a PyO3 native module at
`bouncer._bouncer_native`. The binding calls `bouncer-core` directly for lease
semantics and returns pure-Python dataclasses for result objects. The SQLite
extension is a parallel surface for SQL-only callers, not a layer that Python
wraps.

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

Rust wrapper tests prove negative bootstrap behavior, wrapper/core interop,
SQL/Rust interop, borrowed-path commit/rollback, multi-mutator transactions,
semantic stress, transaction-handle behavior, savepoint participation,
transaction/savepoint durability from a fresh connection, savepoint
renew/release, savepoint commit plus outer rollback, and deterministic
explicit-time semantic stress.

Rust tests also build and load the `bouncer-extension` cdylib through rusqlite
and exercise every `bouncer_*` function.

Python tests prove explicit bootstrap, full lifecycle behavior, transaction
commit/rollback coupling, terminal context-manager behavior, parameter binding,
`BouncerError` coverage for core and SQL errors, multi-statement rejection,
begin-failure re-entry, and SQLite-extension interop on one database file.

Contention semantics are primarily proven at the core and extension boundaries.
The wrappers prove thin delegation, interop, and transaction participation
rather than a separate concurrency model.

## Boundaries

Honker remains the generic async substrate for the family. Bouncer is the
lease/coordination primitive that Honker may depend on later.

Language bindings are typed wrappers over `bouncer-core` where rich typed
results matter. The SQLite extension remains the SQL-only surface. Package
registry publishing is not part of the current baseline.
