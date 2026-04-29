# Reviews And Decisions

This file is append-only.

- Review rounds are written in Session B.
- Decision rounds are written in Session A in response to findings.
- Do not rewrite earlier review text to make it look resolved.

## Decision Round 001

### Responding to

- direct human instruction to finish core behavior, harden it, and test
  one binding before moving on
- cross-session discussion that the remaining core ergonomic gap is a
  sanctioned Rust transaction handle, not another docs pass or another
  binding first
- the existing Honker Rust API shape, which already uses a
  `transaction()` handle as the honest atomic-write path

### Decisions

- [D1] Phase 006 adds a wrapper-owned Rust transaction handle next.
  Target:
  - `spec-diff.md`
  - `plan.md`
  - `ROADMAP.md`

- [D2] Keep this phase smaller than a full ergonomics redesign.
  Ship `Bouncer::transaction()` plus a handle before considering
  `with_transaction(...)` or closure helpers.
  Target:
  - `spec-diff.md`
  - `plan.md`

- [D3] Follow Honker's transaction model where possible.
  The new Bouncer handle should expose lease verbs plus raw connection
  access and explicit `commit` / `rollback`.
  Target:
  - `spec-diff.md`
  - `plan.md`

- [D4] Do not move on to Python or broader docs/examples until the core
  Rust transaction story is settled.
  Target:
  - `ROADMAP.md`

### Verdict

Phase 006 is open. The next correct move is a short Session B review of
the spec diff and plan, then implementation.

## Review Round 001

### Reviewing

- the diff against `packages/bouncer/src/lib.rs` introducing
  `Bouncer::transaction()` and the `Transaction<'db>` handle
- the new wrapper-level tests covering commit, rollback, multi-mutator
  commit and rollback, drop-rollback, and semantic parity
- the ROADMAP next-steps update
- Honker's existing `transaction()` shape in
  `honker/packages/honker-rs/src/lib.rs` for coherence comparison

### Findings

- [F1] Implementation matches the plan tightly. No scope creep.
  `Bouncer::transaction()` returns a `Transaction<'db>` with
  `inspect`/`claim`/`renew`/`release`/`conn`/`commit`/`rollback`. Verbs
  delegate to `core::inspect` and `core::*_in_tx` rather than
  reimplementing semantics. All six tests called for in the plan are
  present.

- [F2] Slight divergence from Honker's transaction shape. Plan said
  "follow Honker's transaction model where possible." Honker uses raw
  `execute("BEGIN IMMEDIATE")` over a `MutexGuard<Connection>` with a
  hand-rolled `Drop`, and exposes verbs as `*_tx` methods on the parent
  type that take `&Transaction`. Bouncer uses
  `rusqlite::Transaction::new_unchecked` and puts verbs directly on the
  handle. Both work; Bouncer's is cleaner because rusqlite owns the SQL
  and Drop. Worth noting because the plan flagged family coherence as a
  concern.

- [F3] `new_unchecked` plus `&self` is intentional and works. The
  trade-off is that calling `wrapper.transaction()` while one is already
  open bubbles a SQLite "cannot start a transaction within a
  transaction" error through the existing `Sqlite(...)` variant. There
  is no Rust-level prevention of double-open. Acceptable, but worth
  knowing.

- [F4] Autocommit verbs on `&Bouncer` silently run inside an open
  transaction. Because `Transaction<'db>` only borrows `&self`, callers
  can hold a tx and also call `wrapper.inspect(...)`, which goes
  through `borrowed()` against the same connection and runs inside the
  active tx without announcing it. Same footgun `BouncerRef` already
  has. Plan deliberately did not redesign this. Worth a SYSTEM.md
  sentence eventually.

- [F5] Each verb inside a transaction calls `system_now_ms()` afresh.
  Multiple lease mutations in one atomic boundary can therefore see
  different `now_ms` values. Consistent with the autocommit path. The
  DST-forward proposal in ROADMAP wants all time through a `Clock`
  seam; this phase keeps the per-call wall-clock reads. Fine for 006,
  relevant when the DST seam lands.

- [F6] `transaction_handle_preserves_lease_semantics` adds another
  `std::thread::sleep(Duration::from_millis(30))` for expiry. ROADMAP's
  hardening pass lists "fragile timing tests" as a target. Same pattern
  as existing tests, not a regression, swept by the upcoming hardening
  phase.

- [F7] `Transaction` does not expose savepoints. Plan did not ask.
  ROADMAP's hardening pass calls out "savepoint symmetry" as future.
  Confirming the intentional gap.

- [F8] `commits.txt` still reads "No git commit has been created for
  this phase yet." Code in `packages/bouncer/src/lib.rs` and
  `ROADMAP.md` is uncommitted. Record the SHA after committing.

### Verdict

The implementation is shippable as 006. None of F2-F7 is blocking;
they are notes for the upcoming hardening phase or for future
ergonomic decisions. F8 is housekeeping for after the commit lands.

## Decision Round 002

### Responding to

- Review Round 001 findings `[F1]` through `[F8]`

### Decisions

- [D5] Accept `[F1]`: no further implementation changes are required
  for Phase 006. The handle shape, tests, and scope all match the
  phase plan closely enough to ship.
  Target:
  - none

- [D6] Record `[F2]` and `[F3]` as accepted shape tradeoffs, not bugs.
  Phase 006 keeps the direct-handle API and `rusqlite::Transaction`
  ownership model even though it diverges a bit from Honker's internal
  implementation shape. The current `Sqlite(...)` error on double-open
  is acceptable for this phase.
  Target:
  - `reviews_and_decisions.md`
  - future hardening / ergonomics discussion

- [D7] Carry `[F4]` into baseline docs when Phase 006 closes. The
  current system model should say plainly that wrapper calls on the same
  connection can observe and participate in an already-open transaction.
  Target:
  - future `SYSTEM.md` closeout

- [D8] Defer `[F5]`, `[F6]`, and `[F7]` to the next hardening phase.
  Those are exactly the kinds of follow-ups the roadmap already names:
  clock seam / time handling, fragile sleep-based tests, and savepoint
  symmetry / deeper transaction ergonomics.
  Target:
  - next hardening phase

- [D9] Accept `[F8]` as normal closeout housekeeping.
  Record the implementation SHA in `commits.txt` after the Phase 006
  commit lands.
  Target:
  - `commits.txt`

### Verdict

The correct reaction is to ship Phase 006 as implemented, merge one
small baseline-truth note into `SYSTEM.md` during closeout, and push
the rest into the explicitly upcoming hardening pass instead of
spinning another implementation cycle here.

## Decision Round 003

### Responding to

- follow-up cross-session critique that `[F3]` and `[F4]` are the same
  root issue: `Bouncer::transaction(&self)` plus
  `rusqlite::Transaction::new_unchecked(...)` leaves a real exclusivity
  relationship as a runtime/doc concern instead of a Rust borrow-checked
  guarantee

### Decisions

- [D10] Reopen `[F3]` and `[F4]` as a small in-phase fix, not a deferred
  tradeoff. `Bouncer::transaction()` should take `&mut self`.
  Target:
  - `spec-diff.md`
  - `plan.md`
  - `packages/bouncer/src/lib.rs`

- [D11] Replace `rusqlite::Transaction::new_unchecked(...)` with the
  normal checked `transaction_with_behavior(TransactionBehavior::Immediate)`
  path now that the wrapper can offer `&mut self`.
  Target:
  - `packages/bouncer/src/lib.rs`

- [D12] Keep `[F7]` deferred. Savepoint surface is still a hardening /
  ergonomics phase concern, not a Phase 006 blocker.
  Target:
  - next hardening phase

### Verdict

Phase 006 stays small, but its transaction constructor should tighten
from runtime/documented exclusivity to compile-time exclusivity before
closeout.

## Decision Round 004

### Responding to

- follow-up implementation review noting that the new public transaction
  surface still needed direct `Transaction::renew` coverage,
  direct `Transaction::inspect` coverage, and wrapper README coverage
  for `transaction()` plus the `tx.conn()` escape hatch

### Decisions

- [D13] Accept the missing-surface-proof feedback and keep it inside
  Phase 006. A new public verb surface should have direct tests for its
  top-level verbs, not just indirect coverage through semantic-stress
  scenarios.
  Target:
  - `packages/bouncer/src/lib.rs`

- [D14] Accept the README feedback and keep it inside Phase 006. The
  wrapper README should show the sanctioned `transaction()` path and
  name `tx.conn()` as the business-write escape hatch, including the
  warning not to issue `BEGIN` / `COMMIT` / `ROLLBACK` through it.
  Target:
  - `packages/bouncer/README.md`

- [D15] Defer the remaining concerns into a concrete hardening follow-up
  phase rather than leaving them as free-floating notes:
  - wrapper savepoint surface
  - cross-connection durability proof for the transaction handle
  - fragile sleep-based timing tests
  - `packages/bouncer/src/lib.rs` file-size cleanup
  - documenting/deciding the intentional family-shape divergence from
    Honker's transaction API where needed
  Target:
  - next hardening phase

### Verdict

Phase 006 should close only after the direct transaction-handle tests
 and README example are present. The broader hardening work belongs in a
 dedicated next phase, not as accidental scope creep inside 006.

## Review Round 002

### Reviewing

- a second pass on the same diff against `packages/bouncer/src/lib.rs`
  with `~/.claude/CLAUDE.md`,
  `ai-coding-process/CODING_STANDARDS.md`, and the existing wrapper
  `README.md` / `SYSTEM.md` / `CHANGELOG.md` / `ROADMAP.md` loaded
- `BouncerRef` test coverage for shape comparison, including
  `borrowed_claim_and_renew_commit_with_explicit_transaction`

### Findings

- [F9] `packages/bouncer/src/lib.rs` is now 1106 lines. Phase 006
  added roughly 290 lines (the file was ~816 before). The standards
  doc is explicit: files under 1000 lines. The bulk is the test
  module. Cleanest fix is to move tests into a sibling module file
  under `packages/bouncer/src/`, but that belongs in its own cleanup
  commit, not 006.

- [F10] `Transaction::renew` has no direct test. The semantic-parity
  test walks `claim → busy → expiry → takeover → release → reclaim`,
  which never exercises `renew`. Compare the `BouncerRef` analogue
  `borrowed_claim_and_renew_commit_with_explicit_transaction`. A
  top-level verb on the new handle should not ship without direct
  coverage. Add a test covering happy renew (token unchanged, expiry
  advanced), wrong-owner rejection, and no-lease rejection.

- [F11] `Transaction::inspect` has no direct test. Every verification
  in the new tests reads through `core::inspect(&wrapper.conn, ...)`
  outside the tx, never through `tx.inspect(...)`. Add one test that
  claims inside a transaction and confirms `tx.inspect("scheduler")`
  returns the just-acquired lease before commit.

- [F12] `packages/bouncer/README.md` was named in the plan's "Files
  likely to change" but the diff does not touch it. The README still
  shows only autocommit usage; there is no example of
  `wrapper.transaction()`, no mention of the new handle's verbs, and
  no note that `tx.conn()` is intended for prepared statements and
  business writes rather than raw `BEGIN`/`COMMIT`/`ROLLBACK`. Update
  the README inside this phase.

- [F13] No cross-connection durability check on the new tx path.
  Existing tests verify by reading on the same `Connection` after
  commit. `BouncerRef` tests have the same gap, so this is not a
  regression. Optional for 006, worth adding when comprehensive-test
  pressure is being applied.

- [F14] Atomic-commit staging not yet shaped. `git status` shows
  `lib.rs` and `ROADMAP.md` modified and `.intent/phases/006-...`
  untracked. The existing rhythm is a single phase commit covering
  code + ROADMAP + phase artifacts (excluding `commits.txt`),
  followed by a `Record Phase 006 commit trace` commit. Naming for
  closeout, not blocking.

- [F15] SYSTEM.md and CHANGELOG.md still describe a pre-Phase-006
  baseline. Plan and Decision Round 002 [D7] explicitly defer this
  to closeout. Confirming the deferral.

### Verdict

Phase 006 is shippable in spirit, but `[F10]`, `[F11]`, and `[F12]`
should be folded into the phase before closeout because they are
small, locally scoped, and close gaps the plan itself implied
(`[F10]` and `[F11]` against "comprehensive tests"; `[F12]` against
the plan's own files-to-change list). `[F9]` is a real coding-standard
violation but is best fixed in its own commit. `[F13]` through `[F15]`
stay deferred.

## Decision Round 004

### Responding to

- Review Round 002 findings `[F9]` through `[F15]`

### Decisions

- [D13] Accept `[F10]`. Add `transaction_handle_renew_extends_existing_lease`
  covering owner-match renew, wrong-owner rejection, and no-lease
  rejection inside the transaction handle.
  Target:
  - `packages/bouncer/src/lib.rs`

- [D14] Accept `[F11]`. Add `transaction_handle_inspect_returns_live_lease`
  covering `tx.inspect("scheduler")` returning the lease that was just
  claimed inside the same transaction, before commit.
  Target:
  - `packages/bouncer/src/lib.rs`

- [D15] Accept `[F12]`. Update `packages/bouncer/README.md` with a
  `wrapper.transaction()` example showing one business write plus one
  lease mutation, plus a one-line guidance note on `tx.conn()` being
  for prepared statements and business writes rather than raw
  `BEGIN`/`COMMIT`/`ROLLBACK`.
  Target:
  - `packages/bouncer/README.md`

- [D16] Defer `[F9]` to a dedicated cleanup commit outside Phase 006.
  Splitting the test module touches lines that have nothing to do
  with the transaction-handle work and would dilute the phase commit.
  Target:
  - future cleanup commit

- [D17] Defer `[F13]` to the upcoming hardening phase. Cross-connection
  durability is the kind of comprehensive-test pressure that fits
  hardening, not transaction-handle ergonomics.
  Target:
  - next hardening phase

- [D18] Accept `[F14]` and `[F15]` as closeout housekeeping. Phase
  commit shape and SYSTEM.md/CHANGELOG.md updates land at phase
  closeout, not in the implementation pass.
  Target:
  - closeout

### Verdict

Phase 006 reopens for a small, contained pass: the `&mut self`
tightening from Decision Round 003, two new tests for the previously
untested verbs, and a README update. Everything else in Round 002 is
correctly deferred.
