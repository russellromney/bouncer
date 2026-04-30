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
  explicit outcome classes plus post-attempt lease state.
- Transaction-mode rows map to explicit setup helpers for autocommit,
  `BEGIN`, `BEGIN IMMEDIATE`, and savepoints.
- SQLite configuration rows map to file-backed test helpers that can set
  `journal_mode` and `busy_timeout` deliberately.

## Phase decisions

- Keep this phase in Rust tests only. Do not involve Python.
- Prefer file-backed SQLite for the matrix so two-connection cases and
  journal mode cases are real. Each matrix row gets its own fresh
  `tempdir` and database path so WAL state cannot leak across rows.
- Cover the direct core path, SQL extension path, and Rust wrapper path.
- Include `claim_in_tx`, `renew_in_tx`, and `release_in_tx` where the
  transaction-mode distinction is the point of the case.
- Use a small explicit matrix, not generated combinations of every
  option. This should read like a behavior table, not a fuzz harness.
- Use `Connection::busy_timeout(Duration)` consistently for timeout
  setup.
- Treat `busy_timeout = 0` and one small nonzero timeout (50ms) as the
  supported timeout comparison in this phase.
- Treat `journal_mode = WAL` and `DELETE` as the journal-mode comparison
  in this phase. Defer other pragmas unless they surface a real behavior
  difference worth pinning.
- Treat SQLite `BUSY` and `LOCKED` as one accepted lock-failure class
  for rows where platform/version differences make exact code brittle.
  Exact message text is not part of the contract.
- Post-attempt state assertions reuse the Phase 011 approach: public API
  reads for lease behavior, direct table reads only where row shape or
  persistence matters.
- The matrix stands alongside the Phase 011 invariant runner rather than
  extending it. Phase 011 owns generated autocommit state-machine proof;
  Phase 012 owns explicit SQLite posture rows.
- If the matrix exposes a real bug, an in-scope fix is limited to a
  small direct behavior fix such as a helper branch, constant, or
  error-class mapping that does not add a new public type, schema
  shape, or documented semantic surface. Broader or semantic changes
  split into a follow-up phase after a decision round.
- The matrix should live in two files:
  - `bouncer-core/tests/sqlite_matrix.rs` for core + SQL extension rows
  - `packages/bouncer/tests/sqlite_matrix.rs` for wrapper-only rows
- SQL extension rows use in-process `attach_bouncer_functions(&conn)`
  registration rather than `LOAD_EXTENSION` of the built cdylib.
- Default total runtime target for the new matrix is comfortably under
  10 seconds on top of the current Rust suite.

## Proposed implementation approach

Add integration-style Rust test files so the matrix is easy to scan. The
core idea:

1. Build a few shared helpers for file-backed databases, connection
   setup, `busy_timeout`, `journal_mode`, and two-connection
   orchestration.
2. Introduce a tiny explicit expectation enum for row outcomes, for
   example:
   - `Acquired`
   - `LeaseBusy`
   - `SqliteBusyOrLocked`
   - `Released`
   - `Rejected`
3. Represent matrix rows as per-row `#[test]` cases or very small row
   tables that still surface a row name in the test name. Do not hide
   the whole matrix inside one giant loop.
4. For each row:
   - prepare the SQLite posture
   - perform the lease mutation through the chosen surface
   - assert whether the result is lease-level rejection, SQLite
     lock-class failure, or success
   - assert post-operation lease state
5. Use lock-state-driven orchestration rather than sleeps. For the
   timeout rows, let one connection hold the writer lock while the other
   attempts the mutation and observe immediate failure vs bounded-wait
   failure. Do not add an eventual-success-under-timeout row in this
   phase.

The matrix should produce a stable behavioral story rather than maximum
row count. Fewer well-named cases are better than a combinatorial cloud
nobody trusts.

## Pinned matrix cells

Core + SQL extension rows:

1. Autocommit live lease on one connection -> `LeaseBusy`, no mutation.
2. Two connections, deferred `BEGIN` on caller-owned transaction ->
   `SqliteBusyOrLocked` on lock upgrade, no mutation.
3. Two connections, `BEGIN IMMEDIATE` on the contending connection ->
   lock-class failure occurs at transaction open rather than lease
   mutation; later lease state remains unchanged.
4. Savepoint inside an outer transaction -> Bouncer participates in the
   caller-owned boundary without nested-transaction failure.
5. `busy_timeout = 0` under writer contention -> immediate
   `SqliteBusyOrLocked`, no mutation.
6. `busy_timeout = 50ms` under writer contention -> bounded-wait
   `SqliteBusyOrLocked`, no mutation.
7. `journal_mode = WAL` live-lease and contention rows preserve the same
   lease semantics as `DELETE`.
8. `journal_mode = DELETE` baseline row for comparison.

Wrapper-only rows:

9. `Bouncer::transaction()` (`BEGIN IMMEDIATE`) claims writer intent up
   front and avoids deferred lock-upgrade ambiguity in the sanctioned
   wrapper-owned path.
10. `BouncerRef` inside caller-owned deferred `BEGIN` mirrors the core
    `*_in_tx` lock-upgrade behavior rather than inventing wrapper-only
    semantics.

## Build order

1. Add `bouncer-core/tests/sqlite_matrix.rs` and
   `packages/bouncer/tests/sqlite_matrix.rs`.
2. Add file-backed test helpers for shared-DB setup with configurable
   `journal_mode` and `busy_timeout`, with a fresh tempdir per row.
3. Add the small `Expect` enum and shared outcome assertion helpers.
4. Add core-level matrix cases for:
   - autocommit live-lease busy
   - deferred `BEGIN` writer contention producing SQLite busy/locked
   - `BEGIN IMMEDIATE` claiming writer intent up front
   - savepoint participation
   - timeout 0 vs 50ms
   - `WAL` vs `DELETE`
5. Add SQL extension matrix cases for the same distinctions where the
   SQL surface is expected to mirror core behavior.
6. Add wrapper-path matrix cases where wrapper ergonomics change the
   boundary, especially `Bouncer::transaction()` vs `BouncerRef`.
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
- Assertions pin result variants/error class and lease state, not exact
  SQLite error message strings.
- Python tests still pass if production code changes.
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
- Do not share database files across rows; WAL persistence and lock
  state can leak.
- Do not let the wrapper tests become a second independent semantics
  model. They should prove delegation and boundary behavior, not invent
  new lease rules.
- Do not churn all existing named tests into the matrix unless the new
  row is clearly replacing them. It is fine for the matrix to sit
  alongside a few older named regressions where readability is better.
- Do not pull corruption/manual-row cases into this phase. That is Phase
  013.

## Files likely to change

- `bouncer-core/tests/sqlite_matrix.rs`
- `packages/bouncer/tests/sqlite_matrix.rs`
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
- Existing tests may remain as named regressions even if a nearby matrix
  row overlaps them. This phase optimizes for a readable behavior map,
  not a heroic deduplication pass.
- Exact `BUSY` vs `LOCKED` code may vary by SQLite path/version for some
  rows. The accepted contract is the lock-failure class unless a row
  intentionally pins a narrower code.

## Commands

- `cargo test -p bouncer-core`
- `cargo test -p bouncer`
- `make test-rust`
- `make test`

## Ambiguities noticed during planning

- No open planning ambiguities remain after Plan Review 1 response.
