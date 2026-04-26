# Plan

## Goal

Fix the borrowed Rust surface before anything else.

`BouncerRef` currently uses the same mutators as a wrapper-owned
connection, even though the caller may already own a transaction on the
borrowed `Connection`. That is the same mismatch Phase 004 fixed for the
SQL extension.

## Phase outcome

At the end of Phase 005, Bouncer should have:

- a public transaction-aware core surface in `bouncer-honker`
- a runtime guard that rejects `*_in_tx` misuse on autocommit
  connections
- `BouncerRef` mutators that branch on `is_autocommit()`
- proof that borrowed Rust calls can participate in caller-owned
  transactions and savepoints

At the end of Phase 005, Bouncer should not have:

- new lease semantics
- a new SQL helper surface
- a wrapper-owned `transaction()` API yet
- a hidden attempt to abstract SQLite away

## Mapping from spec diff to implementation

The spec diff says the real bug is contract mismatch:

- `BouncerRef` borrows a caller-owned connection
- but its mutators still call the autocommit path unconditionally

So the implementation plan should produce:

1. explicit public `*_in_tx` helpers in `bouncer-honker`
2. fail-fast autocommit guards on that public in-transaction surface
3. `BouncerRef` mutators that choose autocommit vs in-transaction paths
4. wrapper tests that pin commit, rollback, savepoint, multi-mutator,
   and semantic-stress behavior on a
   borrowed connection

## Phase decisions already made

- This fix comes before more docs/examples, more bindings, or Honker
  adoption work.
- Phase 005 fixes `BouncerRef` first instead of introducing a bigger
  transaction-handle API.
- The core transaction-aware helpers should become explicit public Rust
  surface, not stay `pub(crate)`, because `BouncerRef` needs the same
  honest contract today. Any future Honker-family dependency is a
  forward-looking note, not the primary justification.
- `BouncerRef` should align with the SQL extension's autocommit
  branching instead of becoming autocommit-only.

## Proposed approach

### 1. Promote the core in-transaction helpers

`bouncer-honker` already has:

- `claim_in_tx`
- `renew_in_tx`
- `release_in_tx`

This phase should promote them into the public Rust contract with
doc-comments that make the precondition explicit:

- caller owns the transaction or savepoint
- helper does not open or commit a transaction
- lock-upgrade timing follows the caller's outer transaction mode

They should also fail fast if called on an autocommit connection:

- `conn.is_autocommit() == true`
  - return an explicit transaction-state error
- `conn.is_autocommit() == false`
  - proceed with the in-transaction operation

### 2. Fix `BouncerRef`

`BouncerRef` mutators should mirror the SQL extension branch:

- `conn.is_autocommit() == true`
  - call `core::claim` / `renew` / `release`
- `conn.is_autocommit() == false`
  - call `core::claim_in_tx` / `renew_in_tx` / `release_in_tx`

This keeps autocommit behavior intuitive while letting borrowed callers
participate in caller-owned transactions honestly.

### 3. Prove the borrowed path

Add wrapper-level tests for:

- borrowed claim inside explicit transaction + commit
- borrowed claim inside explicit transaction + rollback
- borrowed multi-mutator commit and rollback
- one compact borrowed-path semantic-stress scenario that covers busy /
  takeover / release / reclaim token behavior
- borrowed mutator inside a savepoint
- no nested-transaction failure on the borrowed path anymore

## Build order

### 1. Promote core helper visibility and docs

- make the `*_in_tx` helpers public
- tighten doc comments around transaction ownership
- add a runtime guard that rejects autocommit misuse

### 2. Update the borrowed wrapper path

- branch in `BouncerRef::claim`
- branch in `BouncerRef::renew`
- branch in `BouncerRef::release`

### 3. Add wrapper transaction tests

- explicit transaction commit
- explicit transaction rollback
- multi-mutator commit and rollback
- savepoint path
- compact borrowed-path semantic proof

## Files likely to change

- `.intent/phases/005-borrowed-transaction-contract/*`
- `bouncer-honker/src/lib.rs`
- `packages/bouncer/src/lib.rs`
- `ROADMAP.md`

## Areas that should not be touched

- SQL function names or behavior
- current lease semantics
- `SYSTEM.md`
- `CHANGELOG.md`
- larger docs/examples work

## Risks and assumptions

- The main risk is accidentally widening the core public surface in a
  sloppy way. The doc comments and runtime guard should make the
  contract explicit.
- The second risk is letting wrapper behavior drift from the SQL
  extension model again.
- Bouncer is currently a sibling to Honker conceptually, not a technical
  dependency of Honker or vice versa. Phase 005 should not rely on a
  non-existent current integration to justify its surface.
