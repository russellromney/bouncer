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
