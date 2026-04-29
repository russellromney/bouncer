# Reviews And Decisions

This file is append-only.

- Review rounds are written in Session B.
- Decision rounds are written in Session A in response to findings.
- Do not rewrite earlier review text to make it look resolved.

## Decision Round 001

### Responding to

- accumulated follow-up review notes after Phase 006
- direct human instruction not to lose the remaining important issues
  before moving on

### Decisions

- [D1] Phase 007 is the explicit hardening phase before Python.
  Target:
  - `spec-diff.md`
  - `plan.md`
  - `ROADMAP.md`

- [D2] Savepoint surface, cross-connection durability, fragile timing
  tests, file-size cleanup, and default-surface docs all belong here
  rather than as untracked chat TODOs.
  Target:
  - `spec-diff.md`
  - `plan.md`

- [D3] Honker-shape divergence should be treated as a conscious
  family-coherence question, not an accidental omission.
  Target:
  - `spec-diff.md`
  - `plan.md`

### Verdict

Phase 007 is open. Close Phase 006 first, then harden the core before
starting Python.

## Review Round 001

### Reviewing

- the diff against `packages/bouncer/src/lib.rs` reducing it to a 246-
  line module that wires `Bouncer`, `BouncerRef`, `Transaction`, and a
  new `Savepoint` handle, with the test module split out
- the new `packages/bouncer/src/tests.rs` (687 lines) and
  `packages/bouncer/src/tests_transaction.rs` (404 lines)
- the `packages/bouncer/README.md` "Recommended default surfaces"
  block plus a `transaction()` example and a savepoint snippet
- the unmodified `SYSTEM.md`, `ROADMAP.md`, and `CHANGELOG.md`
- `cargo test -p bouncer -p bouncer-honker` (57/57 passing) and
  `cargo clippy -p bouncer --all-targets` (clean)

### Plan-vs-delivery scorecard

- savepoint surface on `Transaction` — delivered
- cross-connection durability proof for the transaction handle —
  delivered as `transaction_handle_commit_is_visible_to_fresh_connection`
- `lib.rs` file split — delivered (246 / 687 / 404 lines)
- fewer fragile sleep-based timing tests — not delivered
- docs say which surface is the recommended default — README yes,
  `SYSTEM.md` no

Three of five fully landed, one half, one not.

### Findings

- [F1] Plan promise on fragile timing tests is unfulfilled. Spec-diff
  said "fragile sleep-based transaction tests are reduced or replaced
  where practical." Both
  `transaction_handle_preserves_lease_semantics` in
  `tests_transaction.rs` and
  `borrowed_mutators_preserve_lease_semantics_inside_explicit_transaction`
  in `tests.rs` still call `std::thread::sleep(Duration::from_millis(30))`.
  The deterministic alternative is to drive the same expiry assertion
  through `bouncer-honker`'s explicit-time core helpers rather than
  through wall-clock sleep on the wrapper. Plan named this; the diff
  did not address it.

- [F2] `SYSTEM.md` was not updated. The plan and spec-diff explicitly
  call for "wrapper/system docs that say plainly which public surface
  is the recommended default for which use case." The wrapper README
  got the recommended-default block; `SYSTEM.md` still describes the
  pre-007 baseline with no `Bouncer::transaction()`, no `Savepoint`,
  and no recommended-default guidance. This is the same gap pattern
  Phase 006 hit and deferred to closeout (Phase 006 Decision Round
  002 [D7]); Phase 007 listed `SYSTEM.md` as in-scope and still did
  not deliver it.

- [F3] `ROADMAP.md` not updated. The plan listed it as a likely-to-
  change file. The next-steps section still describes 007 as the next
  phase rather than as the phase being closed. Trivial fix at
  closeout.

- [F4] `Savepoint` has a subtle commit/rollback asymmetry compared
  with `Transaction`. `Transaction::commit(self)` and
  `Transaction::rollback(self)` both consume — terminal.
  `Savepoint::commit(self)` consumes (releases the savepoint). But
  `Savepoint::rollback(&mut self)` does not consume — it issues
  `ROLLBACK TO`, leaving the savepoint open, and the caller must still
  drop or call `sp.commit()` to release. The existing test
  `savepoint_handle_rollback_discards_lease_mutation` exposes the
  awkwardness:

      sp.rollback().expect("rollback savepoint");
      assert_eq!(sp.inspect("scheduler").expect("..."), None);
      sp.commit().expect("release savepoint");

  Releasing the savepoint after rolling it back reads strangely. Two
  cleaner shapes:

  1. `Savepoint::rollback(self)` consumes, internally doing
     `ROLLBACK TO` followed by `RELEASE`, then drops. Symmetric with
     `Transaction`. Caller reopens via `tx.savepoint()` if they want
     another nested boundary.
  2. Keep current shape and document explicitly that rollback resets
     to the savepoint's start without releasing, and the caller must
     drop or call `commit()` to release.

  Option 1 reads cleaner for the wrapper user. Option 2 is more
  honest about rusqlite's underlying `Savepoint`. Either is fine but
  one of them should land before closeout.

- [F5] `Savepoint::renew` and `Savepoint::release` have no direct
  test. Both savepoint tests exercise only `claim` and `inspect`.
  This is exactly the same gap pattern Round 002 of Phase 006 caught
  for `Transaction::renew` and `Transaction::inspect` (`[F10]`,
  `[F11]` in the Phase 006 doc). The plan said "savepoint behavior is
  exposed and tested." Verbs that ship without direct test coverage
  are not tested.

- [F6] No test for "savepoint commit then outer transaction rollback
  discards the savepoint write." This is the canonical savepoint
  correctness invariant — `RELEASE` does not make changes durable
  until the outer transaction commits.
  `savepoint_handle_commit_persists_after_outer_commit` covers the
  happy dual; the rollback dual is missing.

- [F7] Cross-connection durability is only proven for `Transaction`,
  not for `Savepoint`. `transaction_handle_commit_is_visible_to_fresh_connection`
  reads from a fresh connection after `tx.commit()`. No equivalent
  for the `tx.savepoint() → sp.claim() → sp.commit() → tx.commit()`
  path. Optional for the phase, but if cross-connection durability
  was worth proving for one transaction surface it is worth proving
  for the new nested one.

- [F8] Module wiring uses `#[path]`. `tests.rs` declares
  `#[path = "tests_transaction.rs"] mod transaction;` because
  non-`mod.rs` files default to looking up child modules in a
  matching directory. Two cleaner options: move both into
  `src/tests/mod.rs` plus `src/tests/transaction.rs`, or rename and
  declare `mod tests_transaction;` directly from `lib.rs`.
  Stylistic, not blocking.

- [F9] `Savepoint::savepoint()` (nested savepoints) is not exposed.
  Plan did not ask. Confirming the intentional gap rather than
  silently widening the surface.

- [F10] Tracking and commit shape not yet staged. `tests.rs`,
  `tests_transaction.rs`, and `.intent/phases/007-core-hardening/`
  are untracked. `commits.txt` is empty. Same closeout housekeeping
  pattern as Phase 006.

### Things checked and fine

- File-size goal cleanly hit.
- Compile-time exclusivity: `Savepoint<'db>` mut-borrows the
  `Transaction`, so callers cannot double-open or commit the outer
  transaction while the savepoint is alive. Same property `&mut self`
  gave the transaction surface in Phase 006.
- Drop-rollback semantics work for both `Transaction` and
  `Savepoint` via rusqlite defaults.
- Verbs uniformly call `core::*_in_tx`; no semantic duplication
  inside the new `Savepoint` impl.
- The README "Recommended default surfaces" block is good.
- `transaction_handle_commit_is_visible_to_fresh_connection` is
  exactly the right shape for the durability claim.

### Verdict

Phase 007 is shippable in spirit, but `[F1]`, `[F2]`, `[F4]`,
`[F5]`, and `[F6]` should be folded into the phase before closeout
because they are either explicit plan promises (`[F1]`, `[F2]`) or
small gaps the plan itself implied (`[F4]` API symmetry, `[F5]`
"exposed and tested", `[F6]` the canonical savepoint correctness
invariant). `[F3]`, `[F7]`, `[F8]`, and `[F10]` are reasonable to
defer to closeout housekeeping or the next phase. `[F9]` is an
intentional gap.

## Decision Round 002

### Responding to

- Review Round 001 `[F1]` through `[F10]`
- direct human instruction that the important hardening issues should
  either land in Phase 007 or be made explicit, not drift into chat

### Decisions

- [D4] Accept `[F1]`. Replace the remaining sleep-based semantic
  transaction tests with deterministic explicit-time `bouncer-honker`
  helper calls at the expiry boundary.
  Target:
  - `packages/bouncer/src/tests.rs`
  - `packages/bouncer/src/tests_transaction.rs`

- [D5] Accept `[F2]`. `SYSTEM.md` must name the wrapper default
  surfaces and the Phase 007 proof baseline before closeout.
  Target:
  - `SYSTEM.md`

- [D6] Accept `[F3]` as closeout housekeeping. Move the hardening pass
  out of roadmap "next steps" once it has landed.
  Target:
  - `ROADMAP.md`

- [D7] Accept `[F4]` with the terminal wrapper shape:
  `Savepoint::rollback(self)` consumes the handle, performs
  `ROLLBACK TO`, releases the savepoint, and mirrors
  `Transaction::rollback(self)`.
  Target:
  - `packages/bouncer/src/lib.rs`
  - `packages/bouncer/src/tests_transaction.rs`

- [D8] Accept `[F5]`. Add direct savepoint tests for `renew` and
  `release`.
  Target:
  - `packages/bouncer/src/tests_transaction.rs`

- [D9] Accept `[F6]`. Add the canonical "savepoint commit plus outer
  rollback discards the lease and business write" proof.
  Target:
  - `packages/bouncer/src/tests_transaction.rs`

- [D10] Accept `[F7]` into this phase rather than deferring it. If
  cross-connection durability matters for the transaction handle, it
  matters for the nested savepoint path too.
  Target:
  - `packages/bouncer/src/tests_transaction.rs`

- [D11] Defer `[F8]`. The `#[path]` test-module wiring is stylistic
  and the file-size goal is already met. Do not churn it in this phase.

- [D12] Keep `[F9]` as an intentional gap. Nested savepoints are a new
  surface and should require their own reason before landing.

- [D13] Accept `[F10]` as commit-trace housekeeping. Record the phase
  implementation SHA after the code/docs commit.
  Target:
  - `.intent/phases/007-core-hardening/commits.txt`

### Verdict

Phase 007 should close after the in-phase fixes, health checks, and
commit trace. The next product phase is the first non-Rust binding,
preferably Python.
