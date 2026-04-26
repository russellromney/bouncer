# Reviews And Decisions

This file is append-only.

- Review rounds are written in Session B.
- Decision rounds are written in Session A in response to findings.
- Do not rewrite earlier review text to make it look resolved.

## Decision Round 001

### Responding to

- direct human instruction that the SQL transaction mismatch with Honker
  should become the next phase

### Decisions

- [D1] Phase 004 is the transactional SQL compatibility phase.
  Target:
  - `spec-diff.md`
  - `plan.md`

- [D2] The target contract is Honker-style transactional SQL, not
  autocommit-only SQL.
  Target:
  - `spec-diff.md`
  - `plan.md`
  - `ROADMAP.md`

- [D3] The SQL surface names and return shapes stay stable in this
  phase. The fix is architectural, not a public rename.
  Target:
  - `spec-diff.md`
  - `plan.md`

### Verdict

Phase 004 is now open. The next correct move is Session B review of the
spec diff and plan.

## Review Round 001

Target:
- spec-diff and plan review

Session:
- B

Model family:
- Claude Opus 4.7

Artifacts reviewed:
- `spec-diff.md`
- `plan.md`
- `.intent/phases/003-sql-surface-decision/reviews_and_decisions.md`
  (specifically Round 1 [A1] which motivated this phase)
- `bouncer-honker/src/lib.rs` (the current `claim`/`renew`/`release`
  implementations and `attach_bouncer_functions`)

Verification reviewed:
- planned test matrix only

### Positive conformance review

- [P1] Phase 004 is a direct, focused response to the Phase 003
  Round-1 [A1] adversarial finding (nested-transaction trap). It
  doesn't bundle, doesn't expand, doesn't rename — it just fixes the
  trap. That is the right shape for a follow-up phase.
- [P2] The fix picks the simpler of the two options Phase 003 [A1]
  laid out: branch on `conn.is_autocommit()` and route to the right
  helper, rather than detect-and-savepoint. Branching is one
  if-statement; savepoints would touch every mutation path. Right
  trade-off given the constraint set.
- [P3] SQL function names, arguments, and return shapes do not
  change. Phase 003's cross-language contract holds. Phase 004
  proves Phase 003 was not a misstep — the contract just needed to
  broaden in scope, not change in shape.
- [P4] Rust public helper behavior is preserved by wrapping the new
  transaction-aware helpers in `BEGIN IMMEDIATE`. Existing 17 core
  tests + 14 wrapper tests should continue to pass without
  modification, and that constraint serves as the regression net.
- [P5] "What does not change" carries Phase 001/003 invariants
  forward cleanly: no lease-semantic drift, no implicit `now()`, no
  new bindings, no scheduler/workflow growth. Discipline is intact.
- [P6] Plan correctly identifies `conn.is_autocommit()` as the seam
  and explicitly forbids the alternatives ("guessing from error
  strings or trying to catch nested-transaction failures after the
  fact"). That naming alone closes off the most likely wrong
  implementation.

### Negative conformance review

- [N1] The transaction-aware internal helper has no committed
  signature. Plan says "transaction-aware internal helpers that do
  not open their own transaction" but does not say whether they
  take `&Transaction`, `&Connection` with a documented
  pre-condition, or something else. The choice is load-bearing
  because it determines whether downstream embedders can call the
  in-tx helper directly. Pin it: `claim_in_tx(tx: &Transaction,
  ...)` or `claim_locked(conn: &Connection, ...)` with the
  pre-condition stated. Otherwise the implementer chooses and the
  shape becomes an accidental contract.
- [N2] Visibility scope of the new internal helpers is not
  specified. `pub(crate)`, `pub`, or behind a feature flag? The
  SQL registration lives in the same crate so `pub(crate)` would
  suffice for Phase 004's needs. If they're `pub`, that's a new
  public API surface that callers will start depending on, which
  is a real cross-phase commitment. Pin it.
- [N3] The plan's branching rule is correct but loses one
  guarantee. In the autocommit path, `BEGIN IMMEDIATE` acquires a
  RESERVED lock up front, serializing writers cleanly. In the
  in-transaction path, the *caller's* outer transaction may have
  used the default `BEGIN` (which is `BEGIN DEFERRED`). The first
  write inside `bouncer_claim` then tries to upgrade SHARED →
  RESERVED, and that upgrade can fail with `SQLITE_BUSY` against
  another writer. The spec-diff and plan don't acknowledge this
  difference. Worth at least one sentence: "in the
  in-transaction path, lock-upgrade timing follows the caller's
  outer transaction mode, not Phase 001's `BEGIN IMMEDIATE`
  guarantees." A caller using `BEGIN IMMEDIATE` (or `BEGIN
  EXCLUSIVE`) before calling the SQL mutator gets the old
  behavior; a caller using plain `BEGIN` gets the new, weaker
  one. That's a real semantic distinction.
- [N4] Verification list is missing two assertions worth adding:
  - **Multi-mutator transaction.** `BEGIN; SELECT bouncer_claim(A,
    ...); SELECT bouncer_claim(B, ...); ROLLBACK;` leaves *both*
    undone (and the symmetric COMMIT case preserves both). Real
    callers will batch multiple lease operations in one
    transaction; the current verification list only proves
    one-mutator-per-txn.
  - **Read functions inside transactions.** Plan says "keep
    `inspect` / `owner` / `token` simple" but does not require a
    test that they work inside an explicit transaction. They
    should — but if untested, a future regression that adds a
    write to one of them (e.g. opportunistic GC) would silently
    re-introduce the nested-transaction trap on a code path the
    test matrix doesn't cover.
- [N5] No test is pinned for the contention story under the new
  in-transaction path. Spec-diff and plan don't say what happens
  when two callers each open `BEGIN DEFERRED` and call
  `bouncer_claim` concurrently. The answer is "second one gets
  `SQLITE_BUSY` from the lock upgrade," which is fine — but
  unproven. At minimum, name the expected behavior in the
  spec-diff so a future contention regression is detectable.

### Adversarial review

- [A1] **Savepoints.** SQLite's autocommit flag is cleared by
  `BEGIN`/`COMMIT`/`ROLLBACK`, but `SAVEPOINT` outside a
  transaction also implicitly starts one — so `SAVEPOINT bp1;
  SELECT bouncer_claim(...); RELEASE bp1;` would not be in
  autocommit mode by the time the SQL mutator runs. The
  branching code takes the "in transaction" path, which is
  correct. But this case is not tested or even mentioned. Worth
  one savepoint test or one sentence acknowledging the case.
- [A2] **Phase 003 [A2] is now MORE load-bearing.**
  `ctx.get_connection()` returns a connection that's mid-outer-SQL
  and already holds locks. In autocommit mode, the connection has
  just promoted to RESERVED via the inner `BEGIN IMMEDIATE`. In
  in-transaction mode, the connection is mid-caller-transaction
  with whatever locks the caller's transaction holds, and the
  in-tx helper is about to write through that same handle. SQLite
  tolerates this within one connection by design, but Phase 003
  [A2] said "worth a one-line comment near
  `attach_bouncer_functions` explaining the assumption — or a
  test that pins it." That ask is now stronger because Phase 004
  introduces a second lock-state path. The comment or test should
  land in this phase, not deferred.
- [A3] **Lease-semantics drift defense is described but not
  enforced.** Plan lists "accidentally changing lease semantics" as
  the biggest risk but doesn't say HOW the implementation will
  defend against it. The cleanest defense: the existing 17 core +
  14 wrapper tests must pass byte-identically post-refactor, and
  the new SQL transactional tests should not just prove "the API
  works in transactions" but should re-run a known
  semantic-stress scenario (claim → busy → expired → takeover →
  release → reclaim) inside an explicit transaction and assert
  the same end-state. The plan currently lists "rollback drops
  the lease mutation" and "commit preserves it" — that proves
  the transactional plumbing, not that the lease state machine
  still produces the same outcomes when invoked from within a
  transaction. Worth pinning the regression net explicitly.
- [A4] **The SQL function being called from contexts that are
  neither autocommit nor inside an explicit transaction.** Same
  family as [A1]. Worth either an explicit "the only two states
  we recognize are `is_autocommit() == true` and `false`; the
  branching is exhaustive by SQLite's flag definition" sentence,
  or one test that pins the savepoint case.
- [A5] **DST roadmap implication.** The deterministic-simulation
  roadmap entry treats each lease operation as an atomic unit.
  Phase 004 introduces "transaction containing multiple bouncer
  mutations" as a new caller pattern. If a future DST harness
  wants to interleave operations across simulated processes, it
  now has to treat caller-side transactions as un-interleavable
  units rather than scheduling individual `bouncer_*` calls. Not
  blocking — but Phase 004 narrows the simulator's freedom in a
  way the roadmap entry doesn't anticipate. Worth a one-line
  follow-up note in the roadmap when this phase closes.

### Review verdict

- Accepted with three pins before coding.

Phase 004 is the right phase to do next, the right size, and the
right scope. The fix path (`conn.is_autocommit()` branching) is the
correct choice. Three things should land in the spec-diff or plan
before implementation begins:

1. **[N1]** + **[N2]**: pin the signature and visibility of the new
   transaction-aware helpers. `claim_in_tx(tx: &Transaction, ...)`
   with `pub(crate)` is the natural shape; the choice should be
   committed, not implementer's discretion.
2. **[N3]**: acknowledge in the spec-diff that the in-transaction
   path inherits the caller's outer-transaction lock-upgrade
   timing. Callers who want Phase 001's `BEGIN IMMEDIATE`
   guarantees inside their own transaction must use `BEGIN
   IMMEDIATE` themselves. Otherwise this is a silent semantic
   shift.
3. **[N4]** + **[A3]**: extend the verification list with (a) a
   multi-mutator-per-transaction test (commit and rollback both
   tested), (b) a read-function-in-transaction test, and (c) a
   re-run of a known semantic scenario inside a transaction to
   prove lease semantics didn't drift during the refactor.

[A1]/[A4] (savepoints) is fine to handle with one sentence or one
test; [A2] (Phase 003 [A2] becoming load-bearing) should land the
comment or test that Phase 003 deferred; [N5] (contention naming)
and [A5] (DST roadmap note) are nice-to-haves.

## Decision Round 002

### Responding to

- Review Round 001 findings `[N1]`, `[N2]`, `[N3]`, `[N4]`, and `[A3]`

### Decisions

- [D4] Accept `[N1]` and pin the internal helper shape as
  `*_in_tx(conn: &Connection, ...)`, with the documented precondition
  that the helper does not open or commit a transaction.
  Target:
  - `spec-diff.md`
  - `plan.md`

- [D5] Accept `[N2]` and pin those helpers as `pub(crate)`, not a new
  public API surface.
  Target:
  - `spec-diff.md`
  - `plan.md`

- [D6] Accept `[N3]` and name the lock-upgrade distinction explicitly:
  in the in-transaction SQL path, lock timing follows the caller's
  outer transaction mode.
  Target:
  - `spec-diff.md`
  - `plan.md`

- [D7] Accept `[N4]` and extend the verification matrix with:
  - multi-mutator commit and rollback
  - read-helper-in-transaction proof
  Target:
  - `spec-diff.md`
  - `plan.md`

- [D8] Accept `[A3]` and explicitly require one semantic stress
  scenario inside a transaction so the refactor proves lease semantics,
  not just transaction plumbing.
  Target:
  - `spec-diff.md`
  - `plan.md`

- [D9] Partially accept `[A1]` / `[A4]`: pin savepoints in the plan as
  "must be acknowledged, test optional."
  Target:
  - `plan.md`

- [D10] Accept `[A2]` as an implementation note: Phase 004 should land
  either a short SQL-layer comment or a test that makes the
  `ctx.get_connection()` assumption less ghostly.
  Target:
  - `plan.md`

### Verdict

Phase 004 is accepted for implementation after these plan/spec pins.

## Decision Round 003

### Responding to

- implementation review findings `[N1]`, `[N2]`, `[N3]`, and `[N4]`

### Decisions

- [D11] Accept `[N1]` and add doc comments directly on
  `claim_in_tx`, `renew_in_tx`, and `release_in_tx` so the transaction
  ownership precondition is visible at the call site, not just in the
  phase docs.
  Target:
  - `bouncer-honker/src/lib.rs`

- [D12] Accept `[N2]` and remove the stale Phase 004 roadmap item now
  that the implementation exists locally.
  Target:
  - `ROADMAP.md`

- [D13] Defer `[N3]`. The savepoint rollback case is already pinned;
  the symmetric `SAVEPOINT ... RELEASE` path is useful but not required
  to merge Phase 004 into the baseline.
  Target:
  - future follow-up if the savepoint contract grows

- [D14] Reaffirm `[N4]` as still open. In-transaction contention under
  multiple connections is a real follow-up question, but it is not a
  blocker for Phase 004's transactional SQL contract.
  Target:
  - `ROADMAP.md`
  - future phase if we choose to pin it with dedicated tests

### Verdict

The accepted review follow-ups are now addressed. Phase 004 is ready
for baseline closeout (`SYSTEM.md`, `CHANGELOG.md`, and commit trace)
once we choose to land it.

## Review Round 002

Target:
- implementation review

Session:
- B

Model family:
- Claude Opus 4.7

Artifacts reviewed:
- updated `spec-diff.md` and `plan.md` (post Decision Round 002)
- `bouncer-honker/src/lib.rs` (new `*_in_tx` helpers, refactored
  public `claim`/`renew`/`release`, updated
  `attach_bouncer_functions` with autocommit branching, new
  doc comment on `attach_bouncer_functions`, 7 new transactional
  tests)
- `packages/bouncer/src/lib.rs` (unchanged — verified via
  `git diff`)
- `bouncer-extension/README.md` ("Loading the extension" +
  "Transaction model" sections added)
- `ROADMAP.md` ("Next build steps" updated)
- `commits.txt` (empty per IDD pattern)

Verification reviewed:
- `cargo test`: 14 wrapper + 24 core tests pass (Phase 003 had 17
  core; 7 new tests added — see [P3])
- `cargo build -p bouncer-extension`: builds cleanly
- `cargo clippy --workspace --all-targets`: clean (Phase 003
  [N1] `io_other_error` warning is fixed at line 395 —
  `Error::other` is now in use)

### Positive conformance review

- [P1] Every Decision Round 002 pin is honored in the code:
  - [D4] internal helpers exist as
    `claim_in_tx`/`renew_in_tx`/`release_in_tx` with the
    `(conn: &Connection, ...)` shape (lines 166–289)
  - [D5] all three are `pub(crate)`, not a new public surface
  - [D6] lock-upgrade timing is named in `spec-diff.md` lines
    19–22 AND in the extension README "Transaction model"
    section
  - [D7] multi-mutator commit + rollback + read-helper-in-tx
    tests all present
    (`multiple_sql_mutators_commit_together_inside_explicit_transaction`,
    `multiple_sql_mutators_rollback_together_inside_explicit_transaction`,
    `sql_read_helpers_work_inside_explicit_transaction`)
  - [D8] semantic stress scenario inside transaction →
    `sql_mutators_preserve_lease_semantics_inside_explicit_transaction`
    runs claim → busy → takeover → release → reclaim in one
    BEGIN/COMMIT and pins the token sequence (1, None, 2, 1, 3)
  - [D9] savepoint case is **tested**, not just noted
    (`sql_mutators_work_inside_savepoint_context`) — exceeds
    the "test optional" decision
  - [D10] SQL-layer comment exists at lines 291–307 above
    `attach_bouncer_functions`, explaining both paths AND the
    `ctx.get_connection()` reentrancy assumption from Phase 003
    [A2]
- [P2] The refactor is genuinely a refactor. Public `claim`,
  `renew`, `release` are now three-line wrappers
  (`begin_immediate` → `*_in_tx` → `commit`). The lease-state
  logic moved into the `_in_tx` helpers byte-identically — same
  SQL, same params, same match arms, same result construction.
  No semantic drift visible in diff.
- [P3] **Strong regression evidence.** Phase 003 had 17 core
  tests; Phase 004 has 24 core tests. The 7 new tests are all
  additive (Phase 004-specific transactional behavior). Every
  Phase 001/003 test still passes unmodified. The Rust public
  helpers' tests pass even though their bodies were rewritten
  — that's the cleanest possible proof that the refactor
  preserved semantics.
- [P4] **`packages/bouncer/src/lib.rs` is unchanged** (verified
  via `git diff`). Wrapper behavior preservation is not just a
  claim — it's a code-level fact. 14 wrapper tests pass against
  unchanged wrapper code calling refactored core. This is the
  spec-diff's "Rust wrapper behavior remains unchanged" line
  proven materially.
- [P5] The autocommit-branching code is exactly what the plan
  described. Three places (claim, renew, release), three
  identical `if db.is_autocommit() { ... } else { *_in_tx(...) }`
  blocks, no clever metaprogramming. Easy to read, easy to
  audit.
- [P6] Phase 003 leftover items closed:
  - [N1] (clippy `io_other_error`) — fixed at line 395
  - [A2] (`ctx.get_connection()` reentrancy comment) — landed at
    lines 291–307
  - [N3] (extension README "how to load") — three loading
    methods now documented (sqlite3 CLI, rusqlite, raw C API)
- [P7] Extension README "Transaction model" section is honest
  about the lock-upgrade caveat. Not buried in fine print —
  shown alongside the working examples. A user reading the
  README before deploying will see both the new capability and
  the contention-timing caveat together.
- [P8] `SYSTEM.md` and `CHANGELOG.md` correctly **not** touched
  — proper IDD discipline (baseline updates after acceptance).

### Negative conformance review

- [N1] **The `_in_tx` helpers have no doc comments.** Decision
  Round 002 [D4] pinned the precondition: "the helper does not
  open or commit a transaction and relies on the caller to own
  the transaction boundary." That precondition is in the plan,
  in the spec-diff, and in the SQL-layer comment, but it is
  **not** on the helper functions themselves. Anyone reading
  `bouncer-honker/src/lib.rs:166` sees
  `pub(crate) fn claim_in_tx(conn: &Connection, ...)` with
  zero documentation. A future maintainer who wires a new in-
  process call site (e.g., a future async hook, a future SQL
  function that doesn't go through the autocommit branch) and
  hands it a raw `Connection` in autocommit mode will silently
  lose Phase 001's atomicity — `claim_in_tx` runs two
  statements (SELECT then INSERT/UPDATE) without any
  serializing lock. The blast radius is bounded by `pub(crate)`,
  but the warning belongs at the function, not in
  `plan.md`. **One-line fix per helper:** `/// Precondition:
  caller must hold an open transaction or savepoint on
  `conn`. Does not open or commit one.`
- [N2] **`ROADMAP.md` "Next build steps" is stale.** Item 1
  ("Make `bouncer_claim`...work inside an already-open explicit
  SQL transaction so the extension matches Honker's
  transactional model") is exactly what Phase 004 just
  delivered. Should be removed (or moved to a "completed"
  marker) before the phase closes. Otherwise the roadmap reads
  like the work hasn't happened yet.
- [N3] **The savepoint test only proves the rollback path.**
  `sql_mutators_work_inside_savepoint_context` does
  `SAVEPOINT lease_ops; SELECT bouncer_claim(...); ROLLBACK TO
  lease_ops; RELEASE lease_ops;` and asserts the claim was
  undone. It does not test the symmetric path: `SAVEPOINT;
  SELECT bouncer_claim(...); RELEASE;` (release without
  rollback should commit the savepoint's writes into the parent
  context). For the only-savepoint-no-outer-transaction case,
  RELEASE behaves like COMMIT. Worth one more 6-line test.
- [N4] **Round 1 [N5] (contention under in-transaction path)
  remains unaddressed.** Decision Round 002 didn't accept it,
  and the implementation has no test for it. The spec-diff
  acknowledges the timing shift in prose, but there's no test
  that two parallel `BEGIN; SELECT bouncer_claim(...); COMMIT;`
  flows behave correctly under contention (one claims, the
  other gets `SQLITE_BUSY` on lock upgrade). Not blocking
  because Decision Round 002 explicitly punted it, but flagging
  again so the next phase's reviewer sees it's still open.

### Adversarial review

- [A1] **The `_in_tx` helpers' lack of docs is a footgun in a
  small space.** Phase 004 has effectively two correctness
  classes: the public Rust API (atomic, holds `BEGIN
  IMMEDIATE`) and the in-tx helpers (atomic *only if* the
  caller has a transaction open). The compiler can't catch a
  misuse. `pub(crate)` keeps the misuse inside this crate, but
  the crate already has 1100+ lines and three call sites
  (one per helper, in `attach_bouncer_functions`). One more
  call site added without thinking gives you a silent
  atomicity bug. This is the single most impactful follow-up.
- [A2] **The semantic stress test is excellent regression
  evidence but proves a slightly different thing than it
  claims.** It runs the full state machine inside ONE
  transaction. SQLite's read-your-writes-within-a-transaction
  semantics mean each `claim_in_tx` call sees the previous
  call's writes immediately, so the test exercises the state
  machine correctly. But it does NOT prove that two
  *separate* transactions running the same scenario across
  COMMIT boundaries produce the same end-state. For that you
  need the Phase 001/003 test scenarios, which still pass —
  so the regression net is intact, but the stress test alone
  would not catch a "writes only become visible after
  commit, but the in-tx code path assumes uncommitted reads
  in some way" regression. Worth being precise about what
  the stress test proves: it proves the in-tx helpers work
  correctly with the SQLite-tx isolation model, not that
  they're equivalent to the autocommit path under arbitrary
  interleavings.
- [A3] **The autocommit branching is a runtime decision per
  function call.** Every `bouncer_claim` SQL invocation reads
  `db.is_autocommit()` and dispatches. That's correct, but
  it means the function's behavior depends on connection
  state at call time. A test that opens a transaction, calls
  a series of `bouncer_*` functions, commits, then opens
  another transaction and calls more — should mix the in-tx
  and autocommit paths cleanly. None of the current tests
  exercise the *transition* between modes within one
  connection. Probably fine in practice (SQLite is good at
  this), but unproven.
- [A4] **`Transaction::deref → Connection` is the trick that
  makes this work.** Public `claim` does
  `let tx = begin_immediate(conn)?; claim_in_tx(&tx, ...)`.
  That `&tx` coerces to `&Connection` via Deref. If a future
  rusqlite version changes that Deref impl (unlikely but
  possible — rusqlite has been known to refactor
  Transaction's API), the public path would break in a
  subtle way. Worth a comment near `begin_immediate` noting
  the coercion, or — better — change the public helpers to
  call `claim_in_tx(&*tx, ...)` explicitly so the coercion
  is visible.

### Review verdict

- Accepted with two small follow-up items.

This is a clean, disciplined refactor that delivered exactly what
Decision Round 002 specified, and overshot on Round 1 [A1]/[A4]
by adding a real savepoint test rather than just a note. Every
Round 1 finding the user accepted is materially in the code.
Phase 003's leftover [N1], [A2], and [N3] are all closed.

Two follow-ups before SYSTEM.md update:

1. **[N1]** Add a one-line precondition doc comment to each
   `*_in_tx` helper. The plan and spec-diff already state the
   contract; it just needs to land at the function. ~3 lines
   of code total.
2. **[N2]** Update `ROADMAP.md` "Next build steps" to remove or
   re-mark item 1, which is what Phase 004 just shipped.

Nice-to-haves for follow-up phases:

- **[N3]** one savepoint-RELEASE test to mirror the existing
  savepoint-ROLLBACK test
- **[N4]** the still-open Round 1 [N5] (contention under
  in-transaction path) — explicitly punted in Decision Round
  002, flagged again here so the next phase's reviewer can
  decide whether it's load-bearing
- **[A1]** if the `pub(crate)` boundary ever loosens, the
  precondition needs to become more than a doc comment (e.g.,
  a `Transaction`-typed helper variant)
- **[A4]** make the `Transaction → Connection` deref explicit
  in the call site (`claim_in_tx(&*tx, ...)`)

Phase 004 is shippable; the two follow-ups are about future
maintainers, not current correctness.
