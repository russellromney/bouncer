# Reviews And Decisions

This file is append-only.

- Review rounds are written in Session B.
- Prefer a different model family from Session A when possible.
- Decision rounds are written in Session A in response to findings.
- Do not rewrite earlier review text to make it look resolved.

## Handoff Note

Phase 012 is ready for intent/plan review before implementation.

Suggested next session flow:

1. Read `../idd`.
2. Read `SYSTEM.md`, `ROADMAP.md`, this phase's `spec-diff.md`, and
   `plan.md`.
3. Review the spec diff and plan in this file.
4. Append review findings with stable IDs.
5. Let Session A respond with decisions before coding.

## Intent Review 1

Target:
- spec-diff review

Session:
- B

Model family:
- Claude Opus 4.7. Same family as the likely Session A author;
  cross-family reviewer was not available for this session.

Artifacts reviewed:
- `spec-diff.md`

Verification reviewed:
- not run (intent review only; matrix does not exist yet)
- read `SYSTEM.md` for the current proved baseline (autocommit core
  helpers + `*_in_tx` helpers, SQL extension that mirrors core, Rust
  wrapper with `Bouncer`/`BouncerRef`/`Transaction`/`Savepoint`)
- read `ROADMAP.md` Phase 012 stub (BEGIN, BEGIN IMMEDIATE, savepoints,
  autocommit, two connections, zero/nonzero busy_timeout, journal_mode,
  synchronous, locking_mode, extension loading)

### Positive conformance review

- [P1] The framing is correct. "Pin the line between lease busy and
  SQLite busy/locked" is exactly the distinction Bouncer's contract
  needs to preserve, and the existing tests cover individual cases
  without naming the boundary explicitly. A behavior map closes that
  gap.
- [P2] The three-surface coverage (core, SQL extension, Rust wrapper)
  matches the actual primitive surface area today. Honoring it without
  also adding Python keeps the proved baseline aligned with where
  correctness actually lives.
- [P3] Exclusions are honest and match Phase 011's discipline: no
  corruption, no VFS shims, no fault injection, no shared simulator,
  no binding-footprint expansion. Good fence-posting for 013.
- [P4] The roadmap stub asks for `synchronous`, `locking_mode`, and
  extension-loading coverage; the spec-diff implicitly drops those by
  silence. Defensible — those don't change the lease vs SQLite-lock
  story in obvious ways — but worth making explicit (see [A6]).

### Negative conformance review

- [N1] **Bug-fix escape clause is the same gap Phase 011 closed in
  Review Response 1 ([D7]).** The spec-diff says "Small direct bug
  fixes are in scope; broad or ambiguous semantic changes must split
  into a follow-up phase after a decision round." That language is
  inherited verbatim, which is fine — but "small" is undefined, and
  Phase 011's Implementation Notes section never had to exercise it.
  Phase 012 is much more likely to surface a genuine semantic question
  (e.g., "what should `claim_in_tx` return when the caller's outer
  `BEGIN` couldn't acquire the writer lock?"). Pin a sharper line:
  one-line constant change or error-class relabel is in scope; any
  change to a public type, schema, or documented semantic must split.
- [N2] **Python test continuity isn't asserted.** Phase 011 closed
  this with [D9] ("Python tests still pass if any production core
  code changes"). Phase 012 has the same exposure — `*_in_tx` and
  busy/locked classification could ripple into Python — and currently
  has no acceptance line for it.
- [N3] **"Caller-visible result" is undefined.** The spec-diff says
  the matrix asserts "the returned behavior and the resulting lease
  state." But "returned behavior" can mean (a) the result enum
  variant, (b) the rusqlite error variant, (c) the error message
  text, or (d) all three. SQLite error messages vary across versions
  and platforms; (c) is fragile. Pin (a) and (b) explicitly, and
  document that (c) is intentionally not part of the contract.

### Adversarial review

- [A1] **`SQLITE_BUSY` and `SQLITE_LOCKED` are two different errors,
  not one bucket.** The spec-diff conflates them as "SQLite
  busy/locked." That's correct as a coarse class but the matrix needs
  to either (a) classify each row precisely or (b) declare the class
  itself is the contract. The existing core test
  `deferred_sql_transactions_surface_busy_under_writer_contention`
  matches against either string; that's a precedent for (b). Make it
  a deliberate decision instead of a default.
- [A2] **In-memory vs file-backed isn't pinned at the spec level.**
  The plan recommends file-backed, but the spec-diff is silent. SQLite
  in-memory DBs use a different VFS and have materially different
  lock behavior — most notably, two `:memory:` opens are independent
  databases unless using shared cache, and journal_mode behavior
  differs. Pin file-backed at the spec layer so the negative
  conformance is meaningful (e.g., "no in-memory variants of the
  two-connection cases" rather than letting the plan decide silently).
- [A3] **`journal_mode` persistence semantics are load-bearing.**
  Once a SQLite file is converted to WAL mode, that mode persists
  across re-opens (the `-shm`/`-wal` files exist) until explicitly
  switched. If matrix rows reuse a temp file or fixture across rows,
  WAL state can leak. Pin that each row gets its own fresh temp file,
  or document explicit reset between rows. This is exactly the kind
  of cross-test bleed that produces a flaky ("works in isolation")
  matrix.
- [A4] **`busy_timeout = 0` vs "small nonzero" is asymmetric, and the
  spec-diff doesn't say which side of the asymmetry the matrix
  pins.** With timeout 0, two-connection writer contention produces
  immediate `SQLITE_BUSY`. With nonzero timeout, the second writer
  either succeeds (the first finished within the window) or fails
  after waiting up to `timeout`. Both are valid behaviors. The matrix
  needs to choose: does the nonzero case prove "eventual success when
  the first writer completes quickly" or "failure after bounded
  wait"? Or both, in two separate rows? Pin before code.
- [A5] **The `*_in_tx` surface is implicitly in scope but not
  named.** Phase 011 explicitly punted `claim_in_tx`/`renew_in_tx`/
  `release_in_tx` to 012 (per [D4]). The 012 spec-diff lists "core
  calls" without naming `*_in_tx`. The plan does mention them, so
  there's no real ambiguity to a reader who reads both. But the
  spec-diff is the contract, and the contract should explicitly fold
  `*_in_tx` in — including the case "`*_in_tx` called inside a
  caller-owned `BEGIN` that can't upgrade the writer lock."
- [A6] **The roadmap stub names `synchronous`, `locking_mode`, and
  extension-loading; the spec-diff drops them silently.** Probably
  fine — those don't change the lease vs SQLite-lock story in any
  obvious way. But silent pruning of a roadmap line is exactly the
  kind of intent drift IDD's spec-diff layer exists to prevent.
  Either explicitly defer them ("Phase 014 docs" or "out of scope
  pending evidence of a behavior difference") or fold one
  sanity-check row in.
- [A7] **No criterion for "material change."** The spec-diff says
  posture is in scope "where it materially changes the caller-visible
  result." Without a sharper rule, this becomes a judgment call at
  test-writing time, which means the matrix's coverage is
  unreviewable. Pin: a posture row is in scope if changing it changes
  (a) success vs failure, (b) the failure class (lease vs SQLite
  lock), or (c) the post-attempt lease row state. Otherwise it's
  documented behavior, not a matrix row.
- [A8] **Relationship to the Phase 011 invariant runner is
  unaddressed.** Phase 011 already exercises lease semantics over
  thousands of generated sequences in autocommit. Phase 012's matrix
  will re-exercise the same semantics under different SQLite
  postures. Two questions: (1) does Phase 012 extend the runner with
  posture variation, or does it stand entirely alongside? (2) if
  alongside, where does the matrix's coverage of `claim`/`renew`/
  `release` stop and the runner's begin? Worth a short note in the
  spec-diff so the test surface doesn't quietly grow a third copy of
  the same assertions.

### Review verdict

The intent is sound and the framing is right. The blocking-ish gaps
are [N1] (sharper bug-fix line), [N3] (assertion granularity), [A1]
(BUSY vs LOCKED taxonomy), [A2] (in-memory vs file-backed),
[A3] (journal_mode persistence/isolation), [A4] (busy_timeout
asymmetry), and [A8] (relationship to Phase 011 runner). [N2], [A5],
[A6], and [A7] are smaller documentation pins. None is independently
fatal; collectively they are exactly the questions implementation
will answer silently if not pinned now.

## Plan Review 1

Target:
- plan review

Session:
- B

Model family:
- Claude Opus 4.7. Same family as the likely Session A author;
  cross-family reviewer was not available.

Artifacts reviewed:
- `spec-diff.md`
- `plan.md`

Verification reviewed:
- read `SYSTEM.md` and current Rust source layout to assess matrix
  placement
- read `bouncer-core/src/lib.rs` tests for current SQLite-posture
  coverage (`deferred_sql_transactions_surface_busy_under_writer_contention`,
  `mutating_sql_helpers_*`, `sql_mutators_work_inside_savepoint_context`,
  `multiple_connections_*`)
- read `bouncer-core/tests/invariants.rs` to understand the Phase 011
  runner's existing autocommit coverage
- read `packages/bouncer/src/tests.rs` and `tests_transaction.rs`
  references in the plan

### Positive conformance review

- [P5] Build order is concrete and surface-ordered (helpers → core →
  SQL extension → wrapper → mode comparisons), which matches the
  spec-diff's three-surface framing. Reviewable in slices.
- [P6] "Phase decisions" rejects the right traps: no Python, no
  generated combinations, no pragma zoo. Same scope discipline that
  made Phase 011 land cleanly.
- [P7] "Traps" forbids the right anti-patterns: collapsing lease
  busy and SQLite busy into one bucket, wall-clock sleeps, wrapper
  tests inventing new semantics, pulling 013's corruption work in.
- [P8] "Assumptions and risks" honestly flags SQLite error-string
  variability and timeout flakiness. Both are real and the right
  things to surface ahead of writing tests.
- [P9] "Ambiguities noticed during planning" flags the two real
  open questions (centralize-vs-split matrix, wrapper timeout-row
  duplication) instead of silently picking. Right behavior for a
  planner.

### Negative conformance review

- [N4] **Plan inherits the spec-diff's bug-fix language without
  sharpening it.** Same gap as [N1]. Phase 011's [D7] gave a
  workable rule; reuse or refine it here before code.
- [N5] **Plan does not state Python-test continuity acceptance.**
  Same gap as [N2].
- [N6] **"Files likely to change" includes
  `bouncer-core/src/lib.rs` tests AND new split files AND
  `packages/bouncer/src/tests.rs` AND possibly
  `packages/bouncer/tests/`.** That's four plausible homes for the
  matrix. Pick before writing. Phase 011 picked `bouncer-core/tests/
  invariants.rs` deliberately for public-API discipline; the same
  reasoning applies to most of 012. See [A18].

### Adversarial review

- [A9] **Matrix cardinality is unstated.** The spec-diff lists ~6
  SQLite posture axes × 3 surfaces = up to 18 conceptual cells. The
  plan says "small explicit matrix, not generated combinations" but
  never enumerates the cells. Without a target cell list, the matrix
  is whatever lands. Recommended: write the cell list in the plan
  before code — even six rows would be enough to align Session A and
  Session B on coverage.
- [A10] **Expected-outcome taxonomy isn't pre-defined.** A matrix
  test row needs a name for what it expects. Today the codebase has
  ad-hoc `assert!(matches!(result, ClaimResult::Busy(_)))` and string
  matches like `message.contains("database is locked") ||
  message.contains("database is busy")`. The matrix would benefit
  from a small enum the rows reference, e.g.:
  ```
  enum Expect {
      Acquired,
      LeaseBusy,
      SqliteBusyOrLocked,  // either, by spec-diff [A1] decision
      LeaseRejectedNotInTx,
  }
  ```
  Forces every row to declare its expected outcome explicitly, makes
  the matrix readable, and centralizes the BUSY-vs-LOCKED
  classification per [A1].
- [A11] **Two-connection timing/orchestration is unspecified.** The
  matrix needs a deterministic recipe for "connection B's writer
  collides with connection A's open writer." The existing core test
  uses `BEGIN` + a write on A, then `bouncer_claim` on B without any
  sleep. That works because deferred-vs-deferred writer contention is
  immediate. But the matrix will likely need cases like "A holds an
  immediate writer, B uses `busy_timeout = 50ms` and we want to
  observe timeout expiry." Without explicit orchestration (e.g.,
  threads + channels, or a recipe that proves the contention through
  lock state rather than wall time), those rows will flake. Pin the
  orchestration model before writing them.
- [A12] **WAL mode persistence is a real cross-test hazard.** Two
  matrix rows that share a temp file (or that rely on a fixture
  helper that opens the same path twice in sequence) can see WAL
  state from a previous row. Mitigation: every row gets its own
  `tempfile::tempdir()` and its own database path. Make this an
  explicit helper rule in the plan so every row writer follows it.
- [A13] **Cost budget is not estimated.** Phase 011's plan flagged
  budget creep ([D14]). Phase 012 will create dozens of file-backed
  databases and journal-mode switches; each tempdir + connection
  setup is much slower than in-memory. A 50-row matrix at ~10ms/row
  is fine; at ~200ms/row it's a noticeable CI cost. Worth a quick
  ballpark in the plan, even if just "expected total < N seconds."
- [A14] **Plan does not address how the matrix interacts with the
  existing tests.** The current core suite already covers some of
  this: `multiple_connections_share_live_lease_state`,
  `deferred_sql_transactions_surface_busy_under_writer_contention`,
  the savepoint and explicit-transaction tests. Will the matrix
  supersede them, complement them, or duplicate them? "Avoid a giant
  churn refactor" is wise but doesn't answer the question. Pin
  whether existing tests stay, get reframed as matrix rows, or
  remain as named regressions alongside the matrix.
- [A15] **Relationship to Phase 011 runner mirrors [A8].** The plan
  could either extend the runner (give it a posture parameter) or
  stand alongside it. Either is fine; pick one. If alongside, draw
  the line: "the runner exercises autocommit; the matrix exercises
  posture; both touch lease semantics."
- [A16] **`busy_timeout` setup path isn't pinned.** rusqlite exposes
  both `Connection::busy_timeout(Duration)` and `PRAGMA busy_timeout
  = N`. They have similar but not identical semantics, and existing
  code uses `busy_timeout(Duration::from_millis(0))`. Pick one and
  use it consistently in the matrix; otherwise two rows asserting
  "timeout=0" can be doing different things.
- [A17] **Failure-row state assertions need a model.** The plan says
  "assert post-operation lease state" but doesn't say how — through
  `inspect`/`owner`/`token`, through direct table reads, or both?
  Phase 011 picked both ([D6]). Phase 012 will need the same answer.
  Reuse Phase 011's model rather than reinventing it.
- [A18] **Matrix test file location should be pinned.** The plan
  lists four candidate locations. Recommended split:
  - `bouncer-core/tests/sqlite_matrix.rs` for core + SQL extension
    rows (public-API discipline, integration-test layout)
  - `packages/bouncer/tests/sqlite_matrix.rs` for wrapper-only rows
    (so wrapper-specific failures localize there)
  This mirrors Phase 011's `bouncer-core/tests/invariants.rs` choice
  and keeps the matrix from accidentally reaching into private
  helpers.
- [A19] **No mention of how matrix rows are surfaced in test
  output.** Cargo runs each `#[test]` independently. If the matrix is
  one big function with internal looping, a failure prints an
  index/posture but only one test fails total. If each row is its own
  `#[test]`, failures localize automatically and `cargo test
  --filter` works on row names. Phase 011 used one big generator but
  printed `seed={..} step={..}` to recover replay. The matrix is
  smaller and benefits more from per-row `#[test]`s. Pin the choice.
- [A20] **No mention of the SQL extension cdylib path for matrix
  rows.** The current SQL extension tests use
  `attach_bouncer_functions(&conn)` directly (in-process), not the
  loadable cdylib via `LOAD_EXTENSION`. That's fine for behavior, but
  the spec-diff names "the SQL extension surface" and the cdylib path
  has slightly different lifecycle (per-connection registration on
  load). Pin which path the matrix uses; recommended: in-process
  registration (matches existing tests, faster, deterministic).

### Review verdict

Plan shape is right but defers too many implementation decisions to
test-writing time. The ones that materially block reviewable
implementation are [A9] (matrix cell list), [A10] (Expect enum),
[A11] (two-connection orchestration), [A12] (WAL isolation rule),
[A14] (relationship to existing tests), [A18] (test file location),
and [A19] (per-row vs single-test layout). [A13], [A15], [A16],
[A17], and [A20] are smaller pins. [N4]–[N6] track the same gaps
identified in the intent review.

After Session A pins these in a Review Response, the plan is
implementable as written.

## Review Response 1

Responding to:
- Intent Review 1
- Plan Review 1

Session:
- A

### Inputs

- [N1] sharper bug-fix line
- [N2] Python continuity if production code changes
- [N3] caller-visible assertion granularity
- [A1] `BUSY` vs `LOCKED` classification
- [A2] file-backed vs in-memory at spec level
- [A3] WAL persistence and fresh-tempdir isolation
- [A4] `busy_timeout` asymmetry
- [A5] explicit `*_in_tx` scope in spec
- [A6] silent pruning of `synchronous` / `locking_mode`
- [A7] criterion for a material matrix row
- [A8] relationship to Phase 011 runner
- [A9] explicit matrix cell list
- [A10] expectation taxonomy enum
- [A11] two-connection orchestration model
- [A12] WAL isolation helper rule
- [A13] runtime target
- [A14] relationship to existing tests
- [A15] Phase 011 runner relationship in plan
- [A16] `busy_timeout` setup path
- [A17] post-attempt state assertion model
- [A18] matrix file locations
- [A19] row names / test output clarity
- [A20] SQL extension path selection

### Decisions

- [D1] Accept [N1].
  Action: the bug-fix policy is now sharper in both spec and plan.
  In-scope fixes are limited to small direct behavior fixes that do not
  add a new public type, schema shape, or documented semantic surface.
  Targets: `spec-diff.md`, `plan.md`.

- [D2] Accept [N2].
  Action: acceptance now says Python tests still pass if production code
  changes.
  Targets: `plan.md`.

- [D3] Accept [N3], [A1], and [A10].
  Action: the contract now pins result/error-class assertions rather
  than exact message text, and the plan now requires a small `Expect`
  enum to declare expected row outcomes explicitly.
  Targets: `spec-diff.md`, `plan.md`.

- [D4] Accept [A2], [A3], and [A12].
  Action: the spec now pins file-backed SQLite, and the plan now says
  every row gets a fresh tempdir/database path so WAL state cannot leak.
  Targets: `spec-diff.md`, `plan.md`.

- [D5] Accept [A4].
  Action: Phase 012 will pin two timeout rows only:
  `busy_timeout = 0` immediate lock-class failure and
  `busy_timeout = 50ms` bounded-wait lock-class failure. Eventual
  success under timeout is out of scope for this phase.
  Targets: `spec-diff.md`, `plan.md`.

- [D6] Accept [A5].
  Action: the `*_in_tx` surface is now explicit in the spec as part of
  the caller-owned transaction distinction.
  Targets: `spec-diff.md`.

- [D7] Accept [A6].
  Action: `synchronous` and `locking_mode` are explicitly deferred
  unless implementation finds evidence they change success/failure class
  or post-attempt lease state. Extension loading remains implicitly
  covered by the SQL extension rows.
  Targets: `spec-diff.md`.

- [D8] Accept [A7].
  Action: the spec now defines a material matrix row as one that changes
  success vs failure, lease-level rejection vs SQLite lock-class
  failure, or post-attempt lease state.
  Targets: `spec-diff.md`.

- [D9] Accept [A8] and [A15].
  Action: the plan/spec now explicitly say the matrix stands alongside,
  not inside, the Phase 011 runner. Phase 011 owns generated autocommit
  state-machine proof; Phase 012 owns explicit SQLite posture rows.
  Targets: `spec-diff.md`, `plan.md`.

- [D10] Accept [A9].
  Action: the plan now contains the pinned matrix cells before code.
  Targets: `plan.md`.

- [D11] Accept [A11].
  Action: the plan now pins lock-state-driven orchestration rather than
  sleeps, including the timeout rows.
  Targets: `plan.md`.

- [D12] Accept [A13].
  Action: the plan now pins a rough runtime target of under 10 seconds
  added to the Rust suite.
  Targets: `plan.md`.

- [D13] Accept [A14].
  Action: the plan now treats the matrix as a deliberate new layer that
  may subsume or sit alongside some existing named tests, but forbids a
  giant churn refactor in the same phase.
  Targets: `plan.md`.

- [D14] Accept [A16].
  Action: timeout setup is pinned to `Connection::busy_timeout(Duration)`
  consistently.
  Targets: `plan.md`.

- [D15] Accept [A17].
  Action: post-attempt state assertions reuse the Phase 011 model:
  public API reads for lease behavior, direct table reads only where row
  shape or persistence matters.
  Targets: `plan.md`.

- [D16] Accept [A18] and [A19].
  Action: file locations are pinned to
  `bouncer-core/tests/sqlite_matrix.rs` and
  `packages/bouncer/tests/sqlite_matrix.rs`, and the plan now requires
  named rows/tests so failures localize cleanly.
  Targets: `plan.md`.

- [D17] Accept [A20].
  Action: SQL extension rows use in-process
  `attach_bouncer_functions(&conn)` registration rather than loading the
  built cdylib.
  Targets: `plan.md`.

### Verification

- Updated `spec-diff.md` and `plan.md` with explicit decisions.
- No code changed.
- No tests run; this is an artifact-planning response.

### Decision verdict

Phase 012 is now ready for implementation handoff. The remaining shape
is explicit: file-backed, row-isolated, small matrix, lock-class
taxonomy, no Python, no pragma zoo, and no silent policy decisions left
for code.

## Implementation Notes 1

Session:
- A

Artifacts changed:
- `bouncer-core/tests/sqlite_matrix.rs`
- `packages/bouncer/tests/sqlite_matrix.rs`
- `bouncer-core/src/lib.rs`

### Implemented

- Added a file-backed Phase 012 SQLite behavior matrix for
  `bouncer-core` and the in-process SQL extension surface.
- Added a wrapper-only SQLite behavior matrix for `Bouncer::transaction()`
  and `BouncerRef`.
- Every matrix row creates a fresh tempdir/database path so WAL and lock
  state cannot leak across rows.
- Covered lease busy versus SQLite lock-class failure across:
  - autocommit live-lease busy
  - caller-owned deferred `BEGIN` lock-upgrade contention
  - `BEGIN IMMEDIATE` writer-intent acquisition before mutation
  - savepoint participation inside an outer transaction
  - two connections to one file
  - `busy_timeout = 0`
  - `busy_timeout = 50ms`
  - `journal_mode = DELETE`
  - `journal_mode = WAL`
- Added post-attempt state assertions for every row, using public lease
  reads where lease behavior is the point and direct row reads only for
  row-shape/no-mutation checks.
- Adjusted SQL function error mapping so underlying `rusqlite::Error`
  values from core SQLite failures are returned through the SQL callback
  instead of being eagerly string-wrapped as user-function errors. Non-
  SQLite core errors still return `UserFunctionError`.

### Semantic surprises

- The SQL UDF contention row still surfaces a deferred lock-upgrade
  failure as generic `SQLITE_ERROR` with a lock-class message even after
  the callback returns the underlying `rusqlite::Error`. This appears to
  be SQLite/rusqlite scalar-function boundary behavior rather than a
  Bouncer lease-semantic difference.
- The matrix therefore treats SQL UDF lock failures as the same
  `SqliteBusyOrLocked` class when the returned SQLite code is
  `DatabaseBusy` / `DatabaseLocked` or, for this scalar-function boundary
  only, the generic error carries SQLite's busy/locked text. The full
  error message remains outside the contract.

### Verification

- `cargo test -p bouncer-core --test sqlite_matrix`
  - passed: 13 matrix tests
- `cargo test -p bouncer --test sqlite_matrix`
  - passed: 3 matrix tests
- `cargo test -p bouncer-core`
  - passed
- `cargo test -p bouncer`
  - passed
- `make test-rust`
  - passed
- `make test`
  - passed, including Python: 20 pytest tests

### Closeout status

- `SYSTEM.md` was intentionally not updated. Per IDD and the phase
  instruction, it should wait until implementation review and
  acceptance.
- `CHANGELOG.md`, `ROADMAP.md`, and `commits.txt` were not updated yet
  for the same review-gated closeout reason.

## Implementation Review 1

Target:
- implementation review against `spec-diff.md`, `plan.md`, and
  Review Response 1 decisions [D1]–[D17]

Session:
- B

Model family:
- Claude Opus 4.7. Same family as the likely Session A author;
  cross-family reviewer not available for this session.

Artifacts reviewed:
- `bouncer-core/tests/sqlite_matrix.rs` (new, 475 lines, 13 tests)
- `packages/bouncer/tests/sqlite_matrix.rs` (new, 158 lines, 3 tests)
- `bouncer-core/src/lib.rs` diff (`to_sql_err` rewritten)
- `spec-diff.md` and `plan.md` updated with [D1]–[D17]
- `reviews_and_decisions.md` Review Response 1 + Implementation Notes 1
- existing `bouncer-core/src/lib.rs` test layout for de-duplication
  check

Verification reviewed:
- ran `cargo test -p bouncer-core --test sqlite_matrix` — 13 passed
- ran `cargo test -p bouncer --test sqlite_matrix` — 3 passed
- ran `make test-rust` — full Rust suite passed (core, wrapper,
  extension, both matrix files, Phase 011 invariants)
- `make test` Python verification taken from Implementation Notes
  (20 pytest tests pass) rather than re-run

### Positive conformance review

- [P10] **Per-row test functions match [D16].** Every cell is a
  named `#[test]` so failures localize and `cargo test --filter`
  works on row names. Names follow a consistent
  `<surface>_<posture>_<expected>` pattern that reads as a
  behavior table.
- [P11] **Fresh-tempdir isolation matches [D4]/[A12].** Every test
  builds its own `DbFile::fresh()`. The `_tempdir` field on
  `DbFile` is held for the test lifetime so the directory survives
  the second `Connection::open` while still being cleaned up at
  drop. WAL state cannot leak between rows.
- [P12] **`Expect` enum matches [D3]/[A10].** Both files declare
  `enum Expect { Acquired, LeaseBusy, SqliteBusyOrLocked }` and
  every row uses one. Ad-hoc `assert!(matches!(...))` is gone in
  the matrix code path.
- [P13] **Lock-state-driven orchestration matches [D11]/[A11].**
  No `thread::sleep` anywhere in either file. Two-connection
  contention is proven through `BEGIN`/`BEGIN IMMEDIATE` lock
  ordering. The two timeout rows use `Instant::now()` only for
  bounded-time assertions, not as a sleep barrier.
- [P14] **File locations match [D16]/[A18]:** core + SQL extension
  rows in `bouncer-core/tests/sqlite_matrix.rs`; wrapper rows in
  `packages/bouncer/tests/sqlite_matrix.rs`. Mirrors Phase 011's
  `bouncer-core/tests/invariants.rs` placement.
- [P15] **In-process `attach_bouncer_functions` matches
  [D17]/[A20].** `open_sql_conn` registers in-process. No cdylib
  load path is exercised by the matrix.
- [P16] **`Connection::busy_timeout(Duration)` consistently matches
  [D14]/[A16].** Both timeout rows and the open helpers use it; no
  `PRAGMA busy_timeout` setup is mixed in.
- [P17] **Public-API + direct-row split matches [D15]/[A17].**
  Lease-state checks use `inspect`. Row-shape checks
  (`assert_raw_row`, `assert_no_row`) use direct table reads.
  Phase 011's model is reused, not reinvented.
- [P18] **No churn of existing tests, matching [D13].** The 26
  pre-existing `bouncer-core` lib tests are untouched. The matrix
  sits alongside, exactly as the decision instructed.
- [P19] **Cost target [D12] met.** `cargo test -p bouncer-core
  --test sqlite_matrix` runs in 0.07s; wrapper matrix in 0.01s.
  Adds well under 1s to the suite, far under the 10s budget.
- [P20] **Phase 011 invariants still pass.** The `to_sql_err`
  change rippled correctly: the existing
  `deferred_sql_transactions_surface_busy_under_writer_contention`
  test (which used a string-match on busy/locked) still passes
  unmodified, because the message text remains in the residual
  SQL UDF error case.

### Negative conformance review

- [N7] **Wrapper matrix omits `Savepoint`'s typed surface.** The
  test `bouncer_ref_savepoint_participates_in_caller_owned_boundary`
  exercises raw SQL `SAVEPOINT/ROLLBACK TO/RELEASE` against
  `BouncerRef`, not the wrapper's sanctioned
  `Transaction::savepoint() → Savepoint<'db>` handle described in
  `SYSTEM.md`. The plan's "small representative subset" license
  ([D16]) covers some omissions, but the wrapper's whole reason
  for existing is borrow-exclusivity on `Transaction` and
  `Savepoint`. Either fold one row exercising
  `tx.savepoint().claim(...).commit()` or document explicitly that
  Savepoint contention is covered by the wrapper's existing
  `tests_transaction.rs` and out of matrix scope.
- [N8] **Wrapper matrix omits journal-mode and timeout
  comparisons.** `open_conn` in
  `packages/bouncer/tests/sqlite_matrix.rs:38-45` hard-codes
  `journal_mode = DELETE` and accepts only a `busy_timeout`
  duration; no WAL row, no zero-vs-nonzero pair on the wrapper
  surface. Defensible under [D16]'s subset language, but the spec
  was meant to prove the wrapper "delegates and does not invent
  semantics" under journal/timeout differences. One representative
  WAL wrapper row would close that gap cheaply.
- [N9] **Renew/release surfaces are entirely absent from the
  matrix.** Every row exercises `claim`/`claim_in_tx`/`bouncer_claim`.
  `renew`, `release`, and their `*_in_tx` siblings get zero rows.
  The spec-diff and plan name "claim, renew, release" as the
  matrix surface; the implementation pinned only one. Phase 011's
  runner exercises `renew`/`release` autocommit semantics, but
  not under deferred-vs-immediate or two-connection lock
  contention. At minimum, the `*_in_tx` decision rationale [D6]
  ("[A5] folds `*_in_tx` in") is only half-honored — `claim_in_tx`
  is exercised, the other two are not.
- [N10] **Spec-diff and plan updates land in
  `reviews_and_decisions.md` decisions but the diffs themselves
  weren't reviewed.** Review Response 1 says it updated
  `spec-diff.md` and `plan.md` to reflect [D1]–[D17]. This review
  did not re-read those files post-update, so the artifacts could
  technically have drifted from the decisions. Worth a quick
  spot-check before closeout.

### Adversarial review

- [A21] **`to_sql_err` change is a real production semantic
  change at the SQL extension boundary.** Diff at
  `bouncer-core/src/lib.rs:450-456` rewrites:
  ```
  fn to_sql_err<E: Display>(err: E) -> rusqlite::Error
  ```
  into
  ```
  fn to_sql_err(err: Error) -> rusqlite::Error {
      match err {
          Error::Sqlite(err) => err,
          other => UserFunctionError(...stringified...),
      }
  }
  ```
  Before: every error from a `bouncer_*` SQL UDF surfaced as
  `UserFunctionError("database is busy")` — opaque to
  `ErrorCode`-based callers. After: the underlying
  `SqliteFailure(Busy/Locked, ...)` propagates through, and only
  non-SQLite Bouncer errors get stringified.
  
  Per [D1] this fix qualifies as small (5 lines, no public type
  added, no schema change, no documented surface explicitly
  changed) and is in service of the spec-diff intent. **But**:
  
  - The change is observable to existing SQL callers. A caller
    matching on `UserFunctionError` for busy/locked text will
    silently stop matching. Phase 011 invariants didn't catch
    this because they don't go through the SQL extension. The
    existing `deferred_sql_transactions_surface_busy_under_writer_contention`
    test uses `message.contains("...")` which still passes
    because the residual UDF-boundary error keeps the text — but
    that's coincidence, not contract.
  - SYSTEM.md says "SQL/Rust callers share the same lease state
    machine." The error-mapping behavior at the SQL boundary
    isn't documented in SYSTEM.md, so strictly no documented
    surface is broken. But a reader who trusted the prior
    behavior of "UDF errors come back as UserFunctionError" has
    a regression.
  - **Recommend:** add one line to SYSTEM.md at closeout
    explaining that `bouncer_*` SQL functions surface SQLite
    `BUSY`/`LOCKED` codes natively (via the underlying
    `rusqlite::Error`) and only non-SQLite errors come back as
    `UserFunctionError`. Otherwise this becomes invisible drift.
- [A22] **`sqlite_udf_error_message_is_busy_or_locked` reaches
  into exact message text, which [D3] said is not part of the
  contract.** Lines 118-125 of `bouncer-core/tests/sqlite_matrix.rs`:
  ```
  sqlite_err.code == ErrorCode::Unknown
      && (message.contains("database is busy")
          || message.contains("database is locked"))
  ```
  This is the fallback used when a SQL UDF lock failure crosses
  the rusqlite scalar-function boundary and gets re-classified by
  SQLite as `SQLITE_ERROR` with the original message string.
  
  Implementation Notes 1 acknowledges this honestly:
  > "for this scalar-function boundary only, the generic error
  > carries SQLite's busy/locked text. The full error message
  > remains outside the contract."
  
  But [D3] explicitly pinned that exact message text is *not* the
  contract — and this fallback is exact message text. The pragmatic
  reality is that SQLite/rusqlite collapses the underlying
  `BUSY`/`LOCKED` code at the UDF boundary, so the matrix has no
  other signal. **Recommend:** either
  
  (a) add a `SqliteUnknownWithBusyText` variant to the `Expect`
      enum so the test surface explicitly admits this contract
      hole and forbids new rows from sliding into it, or
  (b) add a phase-decision note that this is a known
      SQLite/rusqlite UDF limitation, the matrix narrows it to a
      single fallback path, and any future regression on this path
      gets escalated rather than fixed by widening the fallback.
  
  Silently keeping the string-match in the helper is the worst
  option — it looks principled until someone adds a second
  string check.
- [A23] **`busy_timeout = 50ms` lower-bound assertion is
  wall-clock dependent.** Line 358:
  ```
  assert!(elapsed >= Duration::from_millis(20), ...)
  ```
  This proves the busy handler waited at least somewhat, which is
  the right intent. But a heavily-loaded CI runner could see the
  busy handler poll, return faster than expected, or get scheduled
  oddly such that 20ms-of-elapsed is not always observed. The
  upper bound (`< 2s`) is generously safe; the lower bound is
  where flakes will live. Bouncer's existing `make test-rust`
  doesn't run on CI yet (per ROADMAP), but if it does, this row
  is the first candidate to flake. Acceptable for V1; flag for
  Phase 014 docs to mention "if this test flakes on slow runners,
  raise the upper bound, never the lower bound."
- [A24] **Two-connection deferred-BEGIN test rolls back conn_b
  before conn_a, which is the only correct order — but the test
  does not document why.** Lines 241-242 / 264-265 / 424-425 do
  `conn_b.ROLLBACK` then `conn_a.ROLLBACK`. If reversed, conn_b's
  rollback would block on conn_a's still-open writer (depending
  on journal mode). The order is right; one comment line above
  the rollback would save a future maintainer five minutes of
  debugging. Minor.
- [A25] **`assert_no_row` after a savepoint test asserts the
  outer transaction committed cleanly with the savepoint rolled
  back, but it does not verify *the savepoint actually rolled
  back its mutation*.** Lines 306-313 / 466-473: `claim_in_tx`
  inserts a row inside the savepoint, then `ROLLBACK TO
  lease_ops; RELEASE; COMMIT`. The post-commit assertion is
  `assert_no_row`. This proves the row is gone, but does not
  prove the savepoint rolled it back vs. the outer commit
  silently dropping it via some other path. The discrimination
  matters because Phase 011's `sql_mutators_work_inside_savepoint_context`
  proves the same thing for SQL. Acceptable as a smoke check;
  flag that the matrix row is a happy-path duplicate of an
  existing test rather than a contention proof under savepoint.
- [A26] **Plan and spec-diff updates were not re-read by this
  review** ([N10]). Decisions [D1]–[D17] are listed, but the
  literal text of `spec-diff.md` and `plan.md` could carry subtle
  drift. **Recommend** Session A spot-check those two artifacts
  before closeout, especially [D10] (matrix cell list literally
  enumerated in plan), [D5] (two timeout rows pinned in spec),
  and [D6] (`*_in_tx` named in spec).
- [A27] **The `to_sql_err` fix is genuinely a Phase 012-driven
  behavior change but Implementation Notes does not declare
  whether it warrants a `CHANGELOG` line distinct from the matrix
  itself.** A future reader of `CHANGELOG.md` may want to know
  that SQL UDF errors changed shape between releases. Worth one
  CHANGELOG bullet at closeout, separate from the matrix bullet.

### Review verdict

The implementation is mostly accepted. Coverage of the pinned cells
is honest, the orchestration is deterministic, file layout matches
the decisions, and the production-code change (`to_sql_err`) is on
the right side of [D1]'s "small fix" line.

The blocking-ish gaps before this can land cleanly:

- [A22] — pin the exact-message fallback as a known
  SQLite/rusqlite-boundary limitation, either via an Expect
  variant or a written decision. Don't leave it as a silent
  string-match in test helpers.
- [A21] / [A27] — at closeout, document in `SYSTEM.md` and
  `CHANGELOG.md` that SQL UDF errors now propagate `BUSY`/`LOCKED`
  natively. Otherwise this is invisible drift.
- [N9] — either fold one row each for `renew_in_tx` and
  `release_in_tx` under deferred contention, or explicitly
  declare them out of matrix scope (covered by Phase 011's
  invariant runner) in the closeout notes.

Smaller pins worth resolving in this round rather than later:

- [N7] — Savepoint-typed-surface coverage in the wrapper matrix.
  One row exercising `tx.savepoint().claim(...).commit()` would
  close it.
- [N8] — one wrapper-side WAL row would prove the wrapper
  doesn't invent semantics under journal-mode differences.
- [N10] / [A26] — spot-check `spec-diff.md` and `plan.md` for
  drift from [D1]–[D17] before closeout.
- [A24] — one comment line on rollback ordering in the
  two-connection deferred tests.
- [A23] — flag the lower-bound timeout assertion to Phase 014
  docs for future flake guidance.

Acceptance posture: accept after [A22], [A21]/[A27]
documentation pins, and an explicit decision on [N9]. Other
findings can land as small follow-up commits inside this phase
or be deferred with a written rationale.

## Review Response 2

Responding to:
- Implementation Review 1

Session:
- A

### Inputs

- [N7] Wrapper matrix omits typed `Savepoint` surface
- [N8] Wrapper matrix omits journal-mode / timeout comparisons
- [N9] `renew_in_tx` and `release_in_tx` get zero matrix rows
- [N10] / [A26] spot-check `spec-diff.md` and `plan.md`
- [A21] / [A27] `to_sql_err` is a production SQL-boundary semantic
  change that needs closeout docs
- [A22] UDF fallback string-match needs an explicit decision
- [A23] `busy_timeout = 50ms` lower-bound assertion could be a future
  flake point
- [A24] deferred two-connection rollback ordering should be documented
- [A25] savepoint row is a happy-path participation proof, not a
  contention proof

### Decisions

- [D18] Accept [A22].
  Action: added an explicit `Expect::SqliteUdfUnknownWithBusyText`
  variant and narrowed the message-text fallback to rows that choose
  that expectation. The generic `SqliteBusyOrLocked` helper now accepts
  only native `DatabaseBusy` / `DatabaseLocked` SQLite codes.

- [D19] Accept [N9].
  Action: added two core matrix rows:
  `core_deferred_begin_renew_in_tx_lock_upgrade_returns_sqlite_lock_class`
  and
  `core_deferred_begin_release_in_tx_lock_upgrade_returns_sqlite_lock_class`.
  Both start from a committed live lease, hold a separate writer lock,
  prove `renew_in_tx` / `release_in_tx` fail with the SQLite lock class
  under deferred lock upgrade, and assert the original lease state is
  unchanged after rollback.

- [D20] Accept [N7] and partially accept [N8].
  Action: added
  `wrapper_typed_savepoint_commit_participates_in_transaction_boundary`
  for the sanctioned `Transaction::savepoint() -> Savepoint` surface,
  and added
  `bouncer_ref_autocommit_live_lease_returns_lease_busy_in_wal_journal`
  as the representative wrapper-side WAL row. I did not add wrapper
  timeout rows because core already owns timeout lock behavior and the
  wrapper rows are meant to prove delegation/boundary behavior, not
  duplicate every SQLite axis.

- [D21] Accept [A24].
  Action: added comments before deferred two-connection rollback pairs
  explaining why the blocked deferred reader rolls back before the
  writer transaction.

- [D22] Accept [N10] / [A26].
  Action: spot-checked `spec-diff.md` and `plan.md` after Review
  Response 1. The decisions [D1]–[D17] are reflected there: file-backed
  fresh tempdirs, accepted lock class, timeout rows, `*_in_tx` scope,
  matrix cell list, file locations, and Python continuity are present.
  No artifact edit was needed.

- [D23] Accept [A21] / [A27] for closeout, not in this response.
  Action: `SYSTEM.md` and `CHANGELOG.md` still remain untouched because
  the user explicitly asked not to update `SYSTEM.md` until
  implementation is reviewed and accepted. At closeout, document that
  SQL functions preserve underlying SQLite `BUSY` / `LOCKED` errors
  where SQLite/rusqlite allows it, while non-SQLite Bouncer errors stay
  `UserFunctionError`.

- [D24] Defer [A23].
  Action: left the `busy_timeout = 50ms` lower-bound assertion in place
  because it is the only row proving nonzero timeout waits before
  bounded failure. If this flakes on future CI, handle it as a Phase 014
  docs/safety-rails note or tune the proof with CI evidence rather than
  widening the fallback silently.

- [D25] Defer [A25].
  Action: kept the savepoint matrix row as a participation proof. The
  deeper savepoint rollback semantics remain covered by existing named
  tests, and the matrix does not need to become a second savepoint test
  suite.

### Verification

- `cargo test -p bouncer-core --test sqlite_matrix`
  - passed: 15 matrix tests
- `cargo test -p bouncer --test sqlite_matrix`
  - passed: 5 matrix tests
- `make test-rust`
  - passed
- `make test`
  - passed, including Python: 20 pytest tests

### Response verdict

Implementation Review 1's blocking gaps are addressed in code or
explicitly pinned for closeout. `SYSTEM.md`, `CHANGELOG.md`,
`ROADMAP.md`, and `commits.txt` are still intentionally unchanged until
the implementation is accepted.

## Closeout 1

Session:
- A

Accepted for closeout by the human after Implementation Review 1 and
Review Response 2.

Artifacts updated at closeout:
- `SYSTEM.md`
- `ROADMAP.md`
- `CHANGELOG.md`
- `.intent/phases/012-sqlite-behavior-matrix/commits.txt`

Closeout summary:

- merged the proved SQLite behavior matrix into the baseline system
  description
- recorded that SQL functions now preserve underlying SQLite
  `BUSY` / `LOCKED` errors where the scalar-function boundary allows it,
  with the known generic-`SQLITE_ERROR` fallback pinned as a boundary
  limitation rather than a lease semantic
- moved the roadmap's next active work to Phase 013
- recorded the current evidence in `commits.txt` pending a landing commit
