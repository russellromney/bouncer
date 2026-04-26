# Reviews And Decisions

This file is append-only.

- Review rounds are written in Session B.
- Decision rounds are written in Session A in response to findings.
- Do not rewrite earlier review text to make it look resolved.

## Decision Round 001

### Responding to

- direct human instruction that the borrowed Rust transaction mismatch
  should be fixed before anything else
- cross-session discussion that `BouncerRef` currently has the same
  nested-transaction trap the SQL extension had before Phase 004

### Decisions

- [D1] Phase 005 fixes the borrowed Rust transaction contract next.
  Target:
  - `spec-diff.md`
  - `plan.md`
  - `ROADMAP.md`

- [D2] Keep this phase small.
  The goal is to align `BouncerRef` with the SQL extension's transaction
  model, not to design the final wrapper ergonomics story all at once.
  Target:
  - `spec-diff.md`
  - `plan.md`

- [D3] Promote the existing `*_in_tx` helpers into explicit public core
  surface rather than inventing a second internal mechanism.
  Target:
  - `spec-diff.md`
  - `plan.md`

- [D4] `BouncerRef` should branch on `is_autocommit()` like the SQL
  extension instead of becoming autocommit-only.
  Target:
  - `spec-diff.md`
  - `plan.md`

### Verdict

Phase 005 is open. Because the design is already fairly pinned, the
next correct move is a short Session B review of the spec/plan rather
than a heavy process loop.

## Decision Round 002

### Responding to

- Review Round 001 findings `[N1]`, `[N2]`, `[N3]`, `[N4]`, and `[A1]`
- cross-session note that Bouncer's Honker relationship is conceptual
  today, not a current technical dependency

### Decisions

- [D5] Accept `[N1]` and `[A1]`: public `*_in_tx` helpers must gain a
  runtime guard that rejects autocommit misuse, not just stronger doc
  comments.
  Target:
  - `spec-diff.md`
  - `plan.md`

- [D6] Accept `[N2]`: add borrowed-path multi-mutator commit and
  rollback tests.
  Target:
  - `spec-diff.md`
  - `plan.md`

- [D7] Accept `[N3]`: add a borrowed-path semantic-stress test rather
  than leaving it to implementer discretion.
  Target:
  - `spec-diff.md`
  - `plan.md`

- [D8] Partially accept `[N4]`: keep the dual-surface shape for this
  phase, but require the plan to name the public rationale more
  explicitly. Do not redesign the API shape in Phase 005.
  Target:
  - `plan.md`

- [D9] Record the naming honesty note: the current Bouncer/Honker
  relationship is conceptual and future-facing, not a current code
  dependency. Phase 005's public-core promotion is justified by
  `BouncerRef` today.
  Target:
  - `plan.md`

### Verdict

Phase 005 is still intentionally small, but the spec and plan now pin
the fail-fast guard, the stronger borrowed-path tests, and the current
truth about the Honker relationship.

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
- `packages/bouncer/src/lib.rs` (current `BouncerRef` implementation)
- `bouncer-honker/src/lib.rs` (existing `pub(crate)` `*_in_tx`
  helpers + their Phase 004 doc comments)
- Phase 004 `reviews_and_decisions.md` Round 002 [A2] and the cross-
  session discussion that surfaced this contract mismatch

Verification reviewed:
- planned test matrix only

### Positive conformance review

- [P1] Phase scope is exactly the latent bug the codex thread
  surfaced: `BouncerRef::claim` calls `core::claim` which opens
  `BEGIN IMMEDIATE`, breaking on caller-owned transactions just
  like the SQL extension did before Phase 004. Phase 005 fixes
  precisely that, no more, no less.
- [P2] The phase chose the smaller intervention deliberately —
  fix `BouncerRef` first, defer the wrapper-owned `with_transaction(|tx|)`
  helper. That matches the "phase shape should be the smallest
  honest fix" pattern Phases 002–004 established.
- [P3] Re-uses Phase 004's existing `*_in_tx` helpers instead of
  inventing a second mechanism. Coherent with Phase 004's design;
  no parallel-taxonomy risk.
- [P4] Mirrors the SQL extension's `is_autocommit()` branching
  byte-for-byte. One mental model ("whoever owns the transaction
  boundary owns the lock-timing") now covers both the SQL surface
  and the borrowed Rust surface.
- [P5] "What does not change" correctly excludes `bouncer_begin()`
  — the right answer per codex's "be explicit where you don't
  own the boundary" principle, captured plainly in the spec-diff.
- [P6] Plan is honest about the surface promotion: `*_in_tx`
  goes from `pub(crate)` to `pub` because wrapper callers and
  potential future Honker callers need the same contract. That's
  a real cross-phase commitment and the plan names it as such
  rather than slipping it in.

### Negative conformance review

- [N1] **Doc comments need to be public-API-grade.** The current
  `*_in_tx` doc text (carried over from Phase 004's `pub(crate)`
  version) reads "Precondition: `conn` is already inside the
  transaction or savepoint that should own atomicity for this
  mutation." That's adequate for an internal helper. For a public
  API, it should also state:
  - what happens if the precondition is violated (silent
    atomicity loss — two statements run without a serializing
    lock; lease state can interleave wrong)
  - the relationship to `Connection::is_autocommit()` —
    specifically that the precondition is equivalent to
    `conn.is_autocommit() == false`
  - that lock-upgrade timing follows the caller's outer
    transaction mode (already in the plan; should be in the doc
    comment too)
  This is the only line of defense against [A1].
- [N2] **Verification list missing a multi-mutator-per-transaction
  test on the borrowed path.** Phase 004 added
  `multiple_sql_mutators_commit_together_inside_explicit_transaction`
  and the rollback variant for the SQL surface. The borrowed Rust
  path should get the equivalent: claim two resources via
  `BouncerRef` inside one BEGIN, COMMIT, prove both are visible;
  symmetric ROLLBACK case proves both are gone. Plan says
  "borrowed release/renew if one compact scenario can cover them
  without bloating the phase" — that punts the multi-mutator
  case. Pin it.
- [N3] **No semantic-stress test for the borrowed path.** Phase
  004 added `sql_mutators_preserve_lease_semantics_inside_explicit_transaction`
  to prove the new branching code didn't drift the state machine.
  The borrowed path is the same kind of refactor and deserves the
  same regression net: claim → busy → takeover → release →
  reclaim, all through `BouncerRef` inside one transaction,
  asserting the token sequence. Plan currently leaves this to
  implementer's discretion ("compact semantic proof if one more
  test materially improves confidence") — it does, and Phase 004
  set the precedent. Commit it.
- [N4] **The "two public functions per operation" shape is now a
  permanent contract.** Promoting `*_in_tx` to `pub` means
  `bouncer-honker` exposes both `claim` and `claim_in_tx`,
  `renew` and `renew_in_tx`, `release` and `release_in_tx` — six
  public functions for three operations, distinguished only by a
  doc-comment precondition. New consumers will ask "which do I
  call?" Documentation can explain; the API shape itself doesn't
  guide the choice. Worth a one-paragraph rationale in the public
  module docs explaining the split, or this becomes a recurring
  question in every future binding/tutorial.

### Adversarial review

- [A1] **The footgun is now public.** With `pub(crate)`, a
  misuse of `claim_in_tx` (calling it on an autocommit
  connection) was contained to the bouncer-honker crate. With
  `pub`, any downstream Rust consumer can hand `claim_in_tx` a
  raw `Connection` in autocommit mode, run two statements without
  a serializing transaction, and silently lose Phase 001's
  atomicity. The doc comment is the only defense; doc comments
  don't fail loud. **Consider a runtime guard at function entry:**
  `if conn.is_autocommit() { return Err(Error::NotInTransaction); }`.
  Cheap, fail-fast, matches the project's existing fail-fast
  patterns (`InvalidTtlMs`, `TtlOverflow`, `TokenOverflow`),
  and turns "silent semantic loss" into "loud type-shaped
  error." A new error variant is a small price for closing this
  hole.
- [A2] **Information-architecture smell, not a Phase 005
  blocker.** Six public functions for three operations is
  workable but not great. Alternative shapes that one mental
  model could collapse into:
  - one set of functions taking an explicit
    `TransactionMode::OpenOwn | TransactionMode::CallerOwned`
    enum
  - a trait `TxScope` implemented by `&Connection` (= "I'll open
    BEGIN IMMEDIATE") and `&Transaction` (= "I won't") that
    `claim<S: TxScope>(scope: S, ...)` dispatches on
  Either is a real surface change with cross-cutting impact.
  Phase 005 explicitly chose to be small; that's correct. But
  this question will return when the future `with_transaction(|tx|)`
  helper or the first non-Rust binding starts to design its API
  shape. Worth noting in the roadmap as a known tension, not
  resolving now.
- [A3] **The `BouncerRef::claim` change is invisible from the
  type system.** `BouncerRef<'a>` borrows `&Connection`. Whether
  the connection is in a transaction is a runtime property the
  Rust compiler cannot see. So `BouncerRef`'s "honest" behavior
  reduces to "every call hits `is_autocommit()` first." That's
  fine — it's the same shape Phase 004 chose for SQL — but it
  means the borrowed path has *zero* compile-time guarantees
  about transaction state. A type-level alternative (`BouncerRef`
  for autocommit, `BouncerInTx<'a>` for borrowed-inside-tx) would
  be safer but it's a larger redesign. Same family as [A2];
  same call to defer.
- [A4] **`Bouncer::claim` is unaffected and that's right.** The
  owned wrapper owns its connection and opens its own transaction
  — `Bouncer::claim` should keep doing exactly what it does. The
  spec-diff and plan correctly leave `Bouncer` alone. Worth
  saying explicitly in the spec-diff "What does not change" so a
  future reader doesn't accidentally widen the phase.

### Review verdict

- Accepted with one change-required and two test additions.

Phase 005 is the right next step, the right size, and aligned with
the cross-surface principle codex articulated. Three pins before
coding:

1. **[N1] + [A1]:** Tighten the public doc comments AND add a
   runtime guard (`Err(Error::NotInTransaction)` if
   `conn.is_autocommit()` at entry to a `*_in_tx`). Doc comments
   alone are insufficient defense for a public API where the
   misuse mode is silent atomicity loss. The guard costs one
   line per helper plus one new `Error` variant.
2. **[N2]:** Add a multi-mutator-per-transaction test on the
   borrowed path (commit + rollback variants), mirroring Phase
   004's coverage.
3. **[N3]:** Add a compact semantic-stress test on the borrowed
   path inside one transaction. Phase 004 set the precedent;
   Phase 005 should match it.

Nice-to-haves: [A4]'s explicit "Bouncer is unchanged" in
spec-diff "What does not change," and [N4]'s one-paragraph module
rationale for the six-functions shape.

Out-of-scope but worth tracking in ROADMAP: [A2] / [A3] — the
broader info-architecture and type-safety question for the
public core surface. Will return when the `with_transaction(|tx|)`
helper or the first non-Rust binding starts shaping its API.

## Review Round 002

Target:
- implementation review

Session:
- B

Model family:
- Claude Opus 4.7

Artifacts reviewed:
- updated `spec-diff.md` and `plan.md` (post Decision Round 002)
- `bouncer-honker/src/lib.rs` (new `Error::NotInTransaction`,
  `ensure_in_tx`, `pub` on three `*_in_tx` helpers, expanded doc
  comments, new `in_tx_helpers_reject_autocommit_connections` test)
- `packages/bouncer/src/lib.rs` (new `is_autocommit()` branching in
  `BouncerRef::claim`/`renew`/`release`, `as_ref` renamed to
  `borrowed`, six new borrowed-path tests)
- working-tree CHANGELOG/ROADMAP/SYSTEM (still carrying Phase 004
  closeout text; no Phase 005 baseline updates yet — correct per
  IDD discipline)

Verification reviewed:
- `cargo test`: 20 wrapper tests + 26 core tests pass
  (Phase 004 had 14 + 25; +6 borrowed-path tests in wrapper, +1
  fail-fast test in core)
- `cargo clippy --workspace --all-targets`: clean

### Positive conformance review

- [P7] Every Decision Round 002 pin is materially in the code:
  - [D5] runtime guard: `Error::NotInTransaction` variant +
    `ensure_in_tx()` helper called at entry to each `*_in_tx`.
    Pinned by `in_tx_helpers_reject_autocommit_connections`.
  - [D6] borrowed-path multi-mutator commit + rollback tests
    present
    (`borrowed_multi_mutator_commit_together_inside_explicit_transaction`,
    `borrowed_multi_mutator_rollback_together_inside_explicit_transaction`).
  - [D7] borrowed-path semantic-stress test present
    (`borrowed_mutators_preserve_lease_semantics_inside_explicit_transaction`)
    — runs the full claim → busy → takeover → release → reclaim
    cycle through `BouncerRef` inside one transaction.
  - [D9] (Honker relationship is conceptual) — captured in plan.
- [P8] **Fail-fast guard pattern is textbook.** `ensure_in_tx`
  joins the existing fail-fast taxonomy (`InvalidTtlMs`,
  `TtlOverflow`, `TokenOverflow`) and converts "silent
  atomicity loss" into "loud type-shaped error." Doc comments
  also reference `Error::NotInTransaction` by name, so a reader
  looking at any of the three helpers sees the failure mode
  spelled out. Closes [A1] cleanly.
- [P9] Doc comments on the public `*_in_tx` helpers now meet
  public-API-grade per [N1]: they state the precondition, the
  `is_autocommit() == false` equivalence, the
  `Error::NotInTransaction` failure mode, that lock-upgrade
  timing follows the caller's outer transaction mode, and that
  the helper does not open or commit a transaction. Three
  matching docblocks, one per helper.
- [P10] **`BouncerRef` branching code mirrors the SQL extension
  byte-for-byte.** Each of three methods is a parallel
  `if self.conn.is_autocommit() { core::X(...) } else {
  core::X_in_tx(...) }`. Easy to read, easy to audit, and the
  shape matches `attach_bouncer_functions`'s SQL-side branches
  exactly. One mental model now covers both the SQL surface and
  the borrowed Rust surface.
- [P11] **`Bouncer::*` correctly unchanged in semantics.** The
  owned wrapper now delegates through `borrowed()` (renamed from
  `as_ref`) which branches on `is_autocommit()`. Since
  `Bouncer::open` produces a fresh autocommit connection that
  the wrapper never puts in a transaction, the branch always
  takes the autocommit path → `core::claim` → `BEGIN IMMEDIATE`.
  Old behavior preserved; same wrapper tests pass unchanged.
- [P12] Bonus fix: `as_ref` → `borrowed` rename closes Phase
  002 [N12] (`AsRef` trait-name shadow). Surface has zero
  external callers; rename was safe to do alongside.
- [P13] **Working-tree discipline is correct.** Phase 005 did
  not touch SYSTEM.md or CHANGELOG.md. CHANGELOG/ROADMAP/SYSTEM
  changes still in the tree are leftover Phase 004 closeouts;
  Phase 005 closeout will land its own entries after acceptance.

### Negative conformance review

- [N5] **Decision Round 002 [D8] only partially landed.** [D8]
  asked for the plan to "name the public rationale more
  explicitly" for the dual-surface (`claim` + `claim_in_tx`,
  etc.) shape. The plan mentions it briefly under "Phase
  decisions already made," but the actual `bouncer-honker`
  crate has no module-level docstring (`//!`) explaining when
  to use which. A future consumer opening the crate sees three
  pairs of public functions with no overview. One-paragraph
  module docstring on `lib.rs` would fix this. Eight or nine
  lines.
- [N6] **The `as_ref` → `borrowed` rename is undocumented in
  Phase 005 artifacts.** Plan doesn't mention it; spec-diff
  doesn't mention it. The rename is the right call (closes
  Phase 002 [N12]) but it's a public-method rename that
  appeared with no decision-round reference. For repo
  archeology purposes, it should land somewhere — either as a
  Decision Round 003 ("noticed during Phase 005, renamed
  alongside"), in `commits.txt`, or in the eventual CHANGELOG
  Phase 005 entry. Pre-launch velocity rules apply (no users,
  break freely), but the rename should still be a fact the
  artifacts record.
- [N7] **Semantic-stress test uses `std::thread::sleep`** at
  line 700-ish (`std::thread::sleep(Duration::from_millis(30))`)
  to expire a 20ms TTL lease. Phase 004's equivalent SQL test
  avoided sleeps by passing explicit `now_ms` values directly
  to the SQL function. The borrowed-path test can't do that
  through the wrapper because the wrapper takes `Duration`, not
  `now_ms` — and `BouncerRef` doesn't expose `*_at` variants
  (Phase 002 chose not to). So the test either needed to (a)
  drop down to `core::claim_in_tx` for the deterministic time
  path, or (b) accept the sleep. The implementation chose (b).
  This is fine in normal CI but is the kind of test that breaks
  on a slow box — a `200ms` reclaim TTL is generous enough that
  most CI will pass, but at scale this is a known-fragile
  pattern.

### Adversarial review

- [A5] **`ensure_in_tx` only catches one footgun.** The guard
  catches "caller forgot to BEGIN," which is the most common
  misuse. It does not catch "caller used a connection that's
  in a transaction owned by a different logical scope" — e.g.,
  if a connection pool returned a connection that was
  mid-transaction from a previous user (which shouldn't happen
  but is a known pool-manager bug class). That's not Phase 005's
  problem to solve, but worth being honest that
  `Error::NotInTransaction` proves "you didn't open one,"
  not "you opened the right one for this work."
- [A6] **Owned `Bouncer::*` now does an extra `is_autocommit()`
  check on every call.** Cost is trivial (it's a flag read on
  the connection struct), but the call path is now
  `Bouncer::claim → borrowed().claim → is_autocommit() check →
  core::claim → BEGIN IMMEDIATE → ...`. Two function calls plus
  one runtime check that we know will always take the same
  branch (`Bouncer` always opens autocommit). Defensible — the
  wrapper stays parallel to `BouncerRef` and a future
  `Bouncer::with_transaction(|tx|)` helper might break the
  invariant. Worth one-line comment near `Bouncer::open`
  documenting the assumption ("connection starts in autocommit;
  `Bouncer` does not open transactions itself; if you want to
  combine writes, get a `borrowed()` ref and BEGIN yourself").
- [A7] **The savepoint-only test (no outer transaction) was
  inherited from Phase 004.** Phase 004 [D13] deferred the
  symmetric `SAVEPOINT ... RELEASE` (commit-equivalent) case;
  Phase 005 inherits the same gap on the borrowed path. The
  Phase 005 test
  (`borrowed_mutators_work_inside_savepoint_context`) only
  exercises ROLLBACK TO. Acceptable since Phase 004 set the
  precedent, but the gap is now in two places.

### Review verdict

- Accepted with one small follow-up before SYSTEM.md update.

Phase 005 implementation matches the spec-diff, plan, and
Decision Round 002 pins materially. The fail-fast guard is
exactly the right shape; doc comments are public-API-grade;
both surfaces (SQL extension and `BouncerRef`) now share one
mental model. 20 + 26 tests pass; clippy clean.

One follow-up:

1. **[N5]** Add a module-level docstring (`//!`) to
   `bouncer-honker/src/lib.rs` (or a short README in the crate)
   explaining when to use `claim` vs `claim_in_tx` (and the
   same for renew/release). Per Decision Round 002 [D8] this
   was supposed to land in this phase. ~8 lines, closes the
   "which one do I call?" recurring question.

Nice-to-haves:

- **[N6]** record the `as_ref` → `borrowed` rename in
  `commits.txt` or in a Decision Round 003 note so the
  archeology is honest
- **[N7]** if/when the semantic-stress test starts flaking on
  CI, switch to direct `core::claim_in_tx` calls with explicit
  `now_ms` to remove the sleep dependency
- **[A6]** one-line comment near `Bouncer::open` documenting
  the autocommit-only invariant

Phase 005 is shippable. The follow-up is about closing the last
Decision Round 002 commitment, not correctness.

## Decision Round 003

### Responding to

- Review Round 002 findings `[N5]`, `[N6]`, `[N7]`, and `[A6]`

### Decisions

- [D10] Accept `[N5]`: add a crate-level `//!` docstring to
  `bouncer-honker/src/lib.rs` explaining when to use the
  transaction-owning surface (`claim` / `renew` / `release`)
  versus the caller-owned transactional surface
  (`claim_in_tx` / `renew_in_tx` / `release_in_tx`).
  Target:
  - `bouncer-honker/src/lib.rs`

- [D11] Record `[N6]` as accepted repo archeology rather than a
  separate code change. The `as_ref` -> `borrowed` rename is now
  explicitly acknowledged in this round and can be mentioned again
  in the eventual Phase 005 changelog entry.
  Target:
  - `reviews_and_decisions.md`
  - future `CHANGELOG.md` closeout

- [D12] Defer `[N7]` and `[A6]`. The sleep-based semantic-stress
  test is acceptable for now, and the extra `is_autocommit()` check
  on `Bouncer::*` is a trivial cost. Revisit only if CI starts
  flaking or Phase 006 redesigns the Rust transaction ergonomics.
  Target:
  - none in this phase

### Verification

- `cargo fmt --all`
- `cargo test`

### Verdict

Phase 005's only required implementation-review follow-up is now
landed. The remaining notes are recorded and intentionally deferred.
