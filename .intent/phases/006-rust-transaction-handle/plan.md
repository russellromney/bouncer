# Plan

## Goal

Finish the sanctioned Rust transaction story before moving on to more
bindings.

Today Bouncer has:

- simple autocommit wrapper calls on `Bouncer`
- an honest borrowed path on `BouncerRef`
- transactional SQL mutators in the extension

What it does not have yet is the obvious Rust answer to:

"How do I combine a business write and a lease mutation in one
`BEGIN IMMEDIATE` boundary without manually juggling `borrowed()` and a
raw `rusqlite::Transaction`?"

This phase should supply that answer.

## Phase outcome

At the end of Phase 006, Bouncer should have:

- `Bouncer::transaction()`
- a transaction handle type in `packages/bouncer`
- lease verbs on that handle that reuse the current public
  `*_in_tx` core surface
- an escape hatch to the underlying SQLite transaction/connection for
  business writes
- tests that pin commit, rollback, drop-rollback, and semantic parity

At the end of Phase 006, Bouncer should not have:

- a new SQL surface
- a closure-based `with_transaction(...)` API yet
- another duplicate lease state machine
- a broader wrapper redesign

## Mapping from spec diff to implementation

The spec diff says the missing piece is wrapper-owned transaction
ergonomics.

So the implementation plan should produce:

1. a wrapper transaction handle that owns a `BEGIN IMMEDIATE`
   `rusqlite::Transaction`
   and holds a mutable borrow of the wrapper's connection while it
   exists
2. lease verbs on that handle that call the public
   `claim_in_tx` / `renew_in_tx` / `release_in_tx` core helpers
3. an escape hatch for business writes in the same atomic boundary
4. tests that prove atomic business-write + lease-mutation behavior

## Phase decisions already made

- This phase comes before another binding because we want the core Rust
  story settled first.
- The shape should follow Honker's existing `transaction()` pattern more
  than inventing a new API family.
- Keep the phase smaller than a full ergonomics rethink:
  `transaction()` plus a handle is enough.
- `BouncerRef` remains as the honest caller-owned connection surface.

## Proposed approach

### 1. Add a wrapper-owned transaction handle

Add a new public type in `packages/bouncer`, likely `Transaction<'db>`.

It should own a `rusqlite::Transaction<'db>` started with
`TransactionBehavior::Immediate`.

`Bouncer::transaction()` should take `&mut self`, not `&self`, so the
wrapper cannot be used normally while the transaction handle is alive on
the same connection.

The handle should expose:

- `inspect(&self, name: &str)`
- `claim(&self, name: &str, owner: &str, ttl: Duration)`
- `renew(&self, name: &str, owner: &str, ttl: Duration)`
- `release(&self, name: &str, owner: &str)`
- `conn(&self) -> &Connection`
- `commit(self)`
- `rollback(self)`

This mirrors the shape Honker already uses.

### 2. Keep the handle thin

The transaction handle should not reimplement lease semantics.

It should:

- use wrapper time conversion helpers
- call `core::inspect`
- call `core::claim_in_tx`
- call `core::renew_in_tx`
- call `core::release_in_tx`

### 3. Prove the transaction story

Add wrapper-level tests for:

- transaction commit with one business write plus one lease mutation
- transaction rollback with one business write plus one lease mutation
- multiple lease mutations inside the same transaction
- drop without commit causing rollback
- semantic parity with the existing autocommit path

## Build order

### 1. Add the handle type and constructor

- add `Bouncer::transaction()`
- add the public transaction handle type
- add `conn()`, `commit()`, and `rollback()`

### 2. Add lease verbs on the handle

- `inspect`
- `claim`
- `renew`
- `release`

### 3. Add tests

- business-write + lease commit
- business-write + lease rollback
- multi-mutator path
- drop rollback
- semantic parity

## Files likely to change

- `.intent/phases/006-rust-transaction-handle/*`
- `packages/bouncer/src/lib.rs`
- `packages/bouncer/README.md`
- `ROADMAP.md`

## Areas that should not be touched

- `bouncer-honker` lease semantics
- SQL function names or behavior
- `SYSTEM.md`
- `CHANGELOG.md`
- Python binding work

## Risks and assumptions

- The main risk is API creep. The phase should stop at one honest
  transaction handle, not a full abstraction layer.
- The second risk is accidentally diverging from Honker's transaction
  mental model. Reusing the same shape should keep the family coherent.
- The third risk is leaving transaction exclusivity as a doc/runtime
  caveat instead of a borrow-checked guarantee. This phase should close
  that gap while keeping the scope small.
- Exposing `conn()` is a deliberate escape hatch. The handle should stay
  thin even if that means callers can still use raw SQL directly.
