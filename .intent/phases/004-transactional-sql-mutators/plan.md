# Plan

## Goal

Make Bouncer's SQL mutators transactional in the Honker sense.

That means a caller should be able to run:

- a business write, and
- `bouncer_claim` / `bouncer_renew` / `bouncer_release`

inside one explicit SQLite transaction on one connection and have the
result commit or roll back together.

## Phase outcome

At the end of Phase 004, Bouncer should have:

- a refactored `bouncer-honker` core with transaction-aware internal
  lease helpers
- unchanged public Rust helper shapes for direct callers
- unchanged public SQL function shapes for extension callers
- proof that SQL mutators work both in autocommit mode and inside an
  already-open explicit transaction

At the end of Phase 004, Bouncer should not have:

- new lease semantics
- SQL function renames
- hidden wall-clock reads
- a second reimplementation of lease logic

## Mapping from spec diff to implementation

The spec diff says the mismatch is between:

- the current Bouncer SQL implementation, which opens nested
  `BEGIN IMMEDIATE` transactions through the public core helpers, and
- the intended Honker-family extension contract, which allows SQL
  helpers to participate in the caller's open transaction.

So the implementation plan should produce:

1. transaction-aware internal lease operations in `bouncer-honker`
2. existing public Rust helpers layered on top of those operations
3. SQL functions that choose the right path based on whether the
   connection is in autocommit mode
4. tests that pin commit/rollback behavior for explicit transactions,
   multi-mutator transactions, read paths inside transactions, and a
   semantic stress scenario inside a transaction

## Phase decisions already made

- This mismatch is real and worth fixing next.
- The target behavior is Honker-style transactional SQL, not
  autocommit-only SQL.
- The SQL surface stays explicit about `now_ms`.
- The SQL surface names and return shapes stay stable.
- The new internal helpers should be `pub(crate)` rather than a new
  public API surface.
- The transaction-aware helper shape is `*_in_tx(conn: &Connection, ...)`
  with a documented precondition: the helper does not open or commit a
  transaction and relies on the caller to own the transaction boundary.
- In the in-transaction SQL path, lock-upgrade timing follows the
  caller's outer transaction mode. Callers who want the old
  `BEGIN IMMEDIATE` behavior inside a SQL transaction must begin that
  outer transaction with `BEGIN IMMEDIATE` themselves.

## Proposed approach

### 1. Split "lease semantics" from "transaction opening"

`bouncer-honker` should have `pub(crate)` internal helpers like:

- `claim_in_tx(conn: &Connection, ...)`
- `renew_in_tx(conn: &Connection, ...)`
- `release_in_tx(conn: &Connection, ...)`

These helpers operate against the current connection state without
opening or committing a transaction. They are not a new public API.

The existing public Rust helpers (`claim`, `renew`, `release`) should
keep their current behavior by:

- starting `BEGIN IMMEDIATE`
- calling the transaction-aware helper
- committing

### 2. Make SQL functions transaction-aware

In the SQL registration layer:

- if `conn.is_autocommit()` is true, preserve the current behavior by
  using the public Rust helper that opens `BEGIN IMMEDIATE`
- if `conn.is_autocommit()` is false, call the transaction-aware helper
  directly on the existing transaction context

This branching is exhaustive for the purposes of this phase: savepoints
also flip SQLite out of autocommit mode, so they should take the same
in-transaction path.

That gives SQL callers the Honker-style contract without weakening the
current Rust convenience story.

### 3. Prove the actual contract

Add tests for:

- SQL mutator success in autocommit mode
- SQL mutator success inside `BEGIN ... COMMIT`
- rollback dropping the lease mutation
- commit preserving the lease mutation
- multiple mutators in one transaction committing and rolling back
  together
- read helpers succeeding inside an explicit transaction
- one semantic stress scenario inside an explicit transaction so the
  lease state machine is proven unchanged, not just the transaction
  plumbing
- the savepoint case either via one explicit test or via a pinned note
  in the SQL-layer comments
- SQL/Rust interop after the refactor

## Build order

### 1. Refactor the core

- extract transaction-aware internal helpers in `bouncer-honker`
- keep public `claim` / `renew` / `release` as wrappers around them

### 2. Update SQL registration

- branch on `conn.is_autocommit()`
- keep `inspect` / `owner` / `token` simple
- make `claim` / `renew` / `release` use the right helper path
- add one short comment near the SQL registration path explaining the
  two lock-state cases and why `ctx.get_connection()` is still expected
  to be valid here

### 3. Add transaction-mode tests

- explicit-transaction commit test
- explicit-transaction rollback test
- multi-mutator commit and rollback tests
- read-helper-in-transaction test
- semantic stress scenario inside a transaction
- prove the old nested-transaction error path is gone
- optionally pin the savepoint case directly if the implementation stays
  simple enough

### 4. Re-verify interop

- ensure wrapper/core/SQL interop still holds on one file

## Files likely to change

- `.intent/phases/004-transactional-sql-mutators/*`
- `bouncer-honker/src/lib.rs`
- `packages/bouncer/src/lib.rs`
- `bouncer-extension/README.md`
- `ROADMAP.md`

## Areas that should not be touched

- Phase 003 SQL function names
- wrapper public method names
- Honker integration
- deterministic-simulation roadmap work

## Risks and assumptions

- The biggest risk is accidentally changing lease semantics while
  extracting transaction-aware helpers.
- The second risk is quietly weakening the autocommit path by removing
  `BEGIN IMMEDIATE` where it still matters.
- `conn.is_autocommit()` is the key seam here; the implementation should
  use it explicitly rather than guessing from error strings or trying to
  catch nested-transaction failures after the fact.
- In the in-transaction path, callers who use plain `BEGIN` inherit
  SQLite's deferred lock-upgrade timing. That can legitimately surface
  `SQLITE_BUSY` at write time under contention. Phase 004 should name
  that behavior, not hide it.
