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
