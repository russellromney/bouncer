# Spec Diff

Phase: 012 — SQLite behavior matrix

Session:
- A

## What changes

- Bouncer gains a table-driven Rust test matrix for the SQLite behaviors
  that materially affect lease outcomes.
- The matrix uses fresh file-backed SQLite databases. In-memory
  databases are out of scope because they do not exercise the same
  locking and journal behavior as real shared files.
- The matrix covers the three Rust/SQLite caller surfaces that own the
  primitive semantics today:
  - direct `bouncer-core` calls
  - the SQL extension surface (`bouncer_*`)
  - the Rust wrapper surfaces (`Bouncer`, `BouncerRef`,
    `Bouncer::transaction()`, and `Savepoint`)
- The matrix explicitly includes the caller-owned transaction helpers
  `claim_in_tx`, `renew_in_tx`, and `release_in_tx` where transaction
  posture is the point of the case.
- The matrix pins the difference between:
  - **lease busy**: a live lease exists, so Bouncer rejects the claim
    without mutating state
  - **SQLite lock-class failure**: SQLite writer-lock acquisition fails
    before Bouncer can complete the mutation
- For this phase, the contract is the lock-failure class
  `SQLITE_BUSY` or `SQLITE_LOCKED`, not exact message text. Tests may
  accept either code where SQLite version/platform differences make that
  necessary.
- The matrix covers these SQLite posture axes where they materially
  change the caller-visible result:
  - autocommit vs `BEGIN`
  - autocommit vs `BEGIN IMMEDIATE`
  - savepoint inside an outer transaction
  - one connection vs two connections to the same file
  - `busy_timeout = 0` vs a small nonzero `busy_timeout`
  - `journal_mode = WAL` vs `DELETE`
- For each pinned case, the tests assert both the returned behavior and
  the resulting lease state after the attempted operation.
- A posture row is in scope if changing it changes one of:
  - success vs failure
  - lease-level rejection vs SQLite lock-class failure
  - post-attempt lease state
- The matrix stands alongside the Phase 011 invariant runner. Phase 011
  proves the explicit-time autocommit lease state machine; Phase 012
  proves how SQLite transaction posture changes the caller-visible
  outcome around that state machine.
- The phase produces a crisp behavioral map for the primitive:
  - when callers get a lease-level rejection
  - when callers get a SQLite lock-class failure
  - when `BEGIN IMMEDIATE` is required to claim writer intent up front
  - when caller-owned transaction mode changes lock-upgrade timing

## What does not change

- No production lease semantics change unless the matrix exposes a real
  bug. In-scope fixes are limited to small direct behavior fixes such as
  a helper branch, constant, or error-class mapping that do not add a
  new public type, schema shape, or documented semantic surface. Broader
  or ambiguous semantic changes must split into a follow-up phase after
  a decision round.
- No schema change.
- No Python binding work. Python remains an example binding, not the
  primary correctness surface for this phase.
- No corruption/manual-row hardening. That belongs in Phase 013.
- No VFS shim, OS fault injection, or distributed simulation harness.
- No package-publishing or binding-footprint expansion.
- No `synchronous` or `locking_mode` matrix rows unless implementation
  finds evidence that they change success/failure class or post-attempt
  lease state. Extension loading is already covered implicitly by the
  SQL extension surface rows.

## How we will verify it

- Add a Rust test matrix that exercises the pinned SQLite posture axes
  and records the expected caller-visible outcome per case.
- Cover at least one case for each of:
  - lease busy in autocommit
  - SQLite busy under deferred transaction writer contention
  - `BEGIN IMMEDIATE` avoiding deferred lock-upgrade ambiguity
  - savepoint participation under an outer transaction
  - `busy_timeout = 0` vs nonzero timeout difference, where timeout 0
    proves immediate lock-class failure and nonzero timeout proves
    bounded-wait lock-class failure
  - `journal_mode = WAL` vs `DELETE` without lease-semantic drift
- `make test-rust` passes.
- `make test` passes if production code changes.

## Notes

- This phase is about caller-visible SQLite behavior, not about
  exhaustively enumerating every pragma SQLite has ever shipped.
- The output should be strong enough that a future docs phase can say
  "if you do X, expect lease busy; if you do Y, expect SQLite
  busy/locked."
