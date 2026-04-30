# Plan

Phase: 012 — SQLite behavior matrix

Session:
- A

## Goal

Harden Bouncer's Rust/SQLite boundary by proving how lease operations
behave under the practical SQLite postures real callers will use. The
point is not more random coverage. The point is to pin the exact line
between "Bouncer rejected this because the lease is busy" and "SQLite
rejected this because the writer lock is busy."

## Context

Phase 011 proved the autocommit lease state machine at a high level
through deterministic explicit-time sequences. Phase 012 moves one layer
outward to the SQLite transaction boundary.

Bouncer already has individual tests for some of this: explicit
transactions, savepoints, and deferred writer contention. What it does
not yet have is one deliberate matrix that explains the behavior across
surfaces and SQLite modes in a way future maintainers can extend without
guessing.

Python is out of scope. The primary correctness surfaces here are
`bouncer-core`, `bouncer-extension`, and the Rust wrapper.

## References

- `SYSTEM.md`
- `ROADMAP.md`
- `.intent/phases/012-sqlite-behavior-matrix/spec-diff.md`
- `bouncer-core/src/lib.rs`
- `packages/bouncer/src/tests.rs`
- `packages/bouncer/src/tests_transaction.rs`

## Mapping from spec diff to implementation

- The behavior matrix maps to table-driven tests grouped by surface and
  SQLite posture.
- The "lease busy vs SQLite busy/locked" split maps to assertions on
  both returned result/error and post-attempt lease state.
- Transaction-mode rows map to explicit setup helpers for autocommit,
  `BEGIN`, `BEGIN IMMEDIATE`, and savepoints.
- SQLite configuration rows map to file-backed test helpers that can set
  `journal_mode` and `busy_timeout` deliberately.

## Phase decisions

- Keep this phase in Rust tests only. Do not involve Python.
- Prefer file-backed SQLite for the matrix so two-connection cases and
  journal mode cases are real.
- Cover the direct core path, SQL extension path, and Rust wrapper path.
- Include `claim_in_tx`, `renew_in_tx`, and `release_in_tx` where the
  transaction-mode distinction is the point of the case.
- Use a small explicit matrix, not generated combinations of every
  option. This should read like a behavior table, not a fuzz harness.
- Treat `busy_timeout = 0` and one small nonzero timeout (for example
  25ms or 50ms) as the supported timeout comparison in this phase.
- Treat `journal_mode = WAL` and `DELETE` as the journal-mode comparison
  in this phase. Defer other pragmas unless they surface a real behavior
  difference worth pinning.
- If the matrix exposes a real bug, a small direct fix is in scope. If
  the fix is broad or changes intended semantics, split into a follow-up
  phase after a decision round.

## Proposed implementation approach

Add or extend Rust test files so the matrix is easy to scan. The core
idea:

1. Build a few shared helpers for file-backed databases, connection
   setup, `busy_timeout`, and `journal_mode`.
2. Represent matrix rows as named tests or small tables, not one giant
   opaque macro.
3. For each row:
   - prepare the SQLite posture
   - perform the lease mutation through the chosen surface
   - assert whether the result is lease-level rejection, SQLite
     busy/locked, or success
   - assert post-operation lease state

The matrix should produce a stable behavioral story rather than maximum
row count. Fewer well-named cases are better than a combinatorial cloud
nobody trusts.

## Build order

1. Add file-backed test helpers for shared-DB setup with configurable
   `journal_mode` and `busy_timeout`.
2. Add core-level matrix cases for:
   - autocommit live-lease busy
   - deferred `BEGIN` writer contention producing SQLite busy/locked
   - `BEGIN IMMEDIATE` claiming writer intent up front
   - savepoint participation
3. Add SQL extension matrix cases for the same distinctions where the
   SQL surface is expected to mirror core behavior.
4. Add wrapper-path matrix cases where wrapper ergonomics change the
   boundary, especially `Bouncer::transaction()` vs `BouncerRef`.
5. Add journal-mode comparison cases (`WAL`, `DELETE`) ensuring no lease
   semantic drift.
6. Add timeout comparison cases showing immediate failure vs bounded
   wait/failure without state corruption.
7. Run Rust tests, then full suite if practical.

## Acceptance

- The phase clearly distinguishes lease busy from SQLite busy/locked for
  the covered cases.
- The covered cases include core, SQL extension, and Rust wrapper
  surfaces.
- The matrix includes at least one verified case each for autocommit,
  deferred transaction, `BEGIN IMMEDIATE`, savepoint, timeout
  difference, and journal-mode difference.
- Post-attempt lease state is asserted for every case.
- `make test-rust` passes.
- `make test` passes if production code changes.

## Tests and evidence

- `cargo test -p bouncer-core`
- `cargo test -p bouncer`
- `make test-rust`
- `make test`

## Traps

- Do not turn this into a pragma zoo. Only pin settings that change the
  caller-visible lease/lock story.
- Do not collapse lease busy and SQLite busy into one generic failure
  bucket. The whole point of the phase is to preserve that distinction.
- Do not rely on wall-clock sleeps where transaction ordering or timeout
  setup can prove the behavior more directly.
- Do not let the wrapper tests become a second independent semantics
  model. They should prove delegation and boundary behavior, not invent
  new lease rules.
- Do not pull corruption/manual-row cases into this phase. That is Phase
  013.

## Files likely to change

- `bouncer-core/src/lib.rs` tests and/or new split core test files
- `packages/bouncer/src/tests.rs`
- `packages/bouncer/src/tests_transaction.rs`
- possibly `packages/bouncer/tests/` if an integration-style matrix is
  clearer there
- `.intent/phases/012-sqlite-behavior-matrix/*`
- `CHANGELOG.md` at closeout
- `SYSTEM.md` at closeout if the matrix meaningfully strengthens the
  proved baseline
- `ROADMAP.md` at closeout

## Areas that should not be touched

- Python binding code or tests
- package publishing or binding footprint
- corruption/manual-row behavior
- shared simulator/harness extraction

## Assumptions and risks

- SQLite error strings can vary slightly by platform/version (`busy` vs
  `locked`). Assertions should be precise enough to catch the class of
  outcome without being fragile to exact wording.
- Timeout-based tests can get flaky if they depend on long sleeps.
  Prefer proving behavior through lock ownership and bounded waits.
- The matrix may reveal that some currently separate tests are
  redundant. That is acceptable if readability improves, but avoid a
  giant churn refactor in the same phase.

## Commands

- `cargo test -p bouncer-core`
- `cargo test -p bouncer`
- `make test-rust`
- `make test`

## Ambiguities noticed during planning

- Whether to centralize the matrix in `bouncer-core` only or split it by
  surface. Initial recommendation: split by surface so failure location
  tells us which boundary moved.
- Whether wrapper-path timeout cases add enough value beyond core/SQL
  cases to justify the runtime. Initial recommendation: include a small
  representative subset, not the whole matrix again.
