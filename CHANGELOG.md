# Changelog

## Unreleased

### Added

- Added the initial repo scaffold for `bouncer`.
- Added the first real `bouncer-honker` Rust core crate implementation.
- Added the first SQLite schema bootstrap for `bouncer_resources`.
- Added Rust `claim`, `renew`, `release`, and time-aware `inspect` helpers.
- Added Phase 001 tests for claim, expiry, renew, release, and monotonic fencing behavior.
- Added the first Rust wrapper crate in `packages/bouncer`.
- Added explicit wrapper bootstrap plus owned/borrowed wrapper types.
- Added wrapper tests for negative bootstrap behavior, wrapper/core interop, TTL parity, and fencing-token monotonicity across wrapper/core calls.
- Added the first SQLite loadable-extension crate in `bouncer-extension`.
- Added the first `bouncer_*` SQL surface:
  - `bouncer_bootstrap()`
  - `bouncer_claim(name, owner, ttl_ms, now_ms)`
  - `bouncer_renew(name, owner, ttl_ms, now_ms)`
  - `bouncer_release(name, owner, now_ms)`
  - `bouncer_owner(name, now_ms)`
  - `bouncer_token(name)`
- Added direct SQL-function tests in `bouncer-honker` and SQL/Rust interop tests in `packages/bouncer`.
- Added transaction-aware internal `*_in_tx` lease helpers in `bouncer-honker`.
- Added explicit-transaction, multi-mutator, read-in-transaction, semantic-stress, and savepoint SQL tests for the extension surface.
- Added the first pass of `README.md`, `ROADMAP.md`, and `SYSTEM.md` to capture product intent before implementation.
- Added `.intent/phases/001-core-lease-contract/` with spec, plan, review/decision, and commit-trace artifacts.

### Changed

- Clarified that Bouncer is a single-machine lease / fencing primitive for SQLite apps, not a distributed coordination system.
- Clarified that Phase 001 stops at the Rust core contract and tests; bindings remain future work.
- Clarified the repo's phase workflow around `spec-diff.md`, `plan.md`, and `reviews_and_decisions.md`.
- Clarified that the Rust wrapper stays thin, keeps bootstrap explicit, and treats wall clock as expiry bookkeeping rather than an ordering primitive.
- Clarified that the SQL surface is now real, keeps `now_ms` explicit, and shares semantics with the Rust core rather than reimplementing lease logic.
- Clarified that SQL mutators now participate in caller-owned explicit transactions and savepoints while preserving the autocommit path's `BEGIN IMMEDIATE` behavior.
