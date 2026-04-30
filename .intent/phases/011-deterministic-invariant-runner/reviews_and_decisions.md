# Reviews And Decisions

This file is append-only.

- Review rounds are written in Session B.
- Prefer a different model family from Session A when possible.
- Decision rounds are written in Session A in response to findings.
- Do not rewrite earlier review text to make it look resolved.

## Handoff Note

Phase 011 is ready for intent/plan review before implementation.

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
- Claude Opus 4.7. Same family as the most likely Session A author;
  cross-family reviewer was not available for this session.

Artifacts reviewed:
- `spec-diff.md`

Verification reviewed:
- not run (intent review only; runner does not exist yet)

### Positive conformance review

- [P1] The "What changes" list is the right minimal shape for a
  first DST-shaped step. Seeded sequences over the six core verbs
  plus model time advancement is exactly what is missing today,
  and nothing more.
- [P2] Invariants are well-chosen. They cover state-machine
  integrity ("at most one live owner per resource"), monotonicity
  ("fencing tokens never decrease"), null effects ("busy claims do
  not mutate state", "wrong-owner renew/release attempts do not
  mutate state"), and read consistency (`inspect`, `owner`, `token`
  agree). These are the load-bearing promises Bouncer makes; if any
  of them fails, Bouncer is broken.
- [P3] The replayability requirement ("seed and operation index")
  is the right primitive for turning a flaky finding into a fixable
  one. Without it, any failure the runner produces is essentially
  unactionable.
- [P4] Scope-fences are honest. Explicitly excluding the SQLite
  settings matrix, lock contention, VFS shims, fault injection, and
  a shared simulator crate keeps Phase 011 small and lets 012/DST
  phases inherit cleanly.

### Negative conformance review

- [N1] "What does not change" claims no production lease semantics
  change, but the runner's job is to *find* drift between the
  intended semantics and the implemented semantics. If a real bug
  surfaces, the spec-diff currently has no escape clause covering
  "we found a real bug and have to change production code in this
  phase." Either declare such a fix is in scope (and split the
  phase if needed) or declare it is out of scope (and require a
  follow-up phase). Silence here will become a fork in the road.
- [N2] The "No SQLite settings matrix" exclusion is correct, but
  the runner will run against *some* SQLite configuration. Pick
  and document it: the in-memory database the plan implies, with
  default pragmas. Otherwise the negative conformance ("no settings
  matrix") is undefined.

### Adversarial review

- [A1] **Invariant scope is fuzzy on observation time.** "At most
  one live owner exists per resource at any observed time" — but
  observed by which surface? `inspect(now_ms)` returns the live
  lease at one moment; the runner can sample many moments. Worth
  tightening: "at every `now_ms` value the runner samples,
  `inspect(now_ms)` returns at most one live owner."
- [A2] **"Successful first claim initializes token 1" is
  ambiguous.** First claim of a resource that has never had a row?
  Or first claim after a release? After a release the token is 1
  but the next claim becomes token 2 (release does not reset).
  Pin: "the first successful claim against a fresh resource
  produces token 1; subsequent successful claims after release or
  expiry produce strictly larger tokens."
- [A3] **`token(resource)` is time-independent in the core, but
  the spec-diff lists it among "agree with each other."** `token`
  returns the row's stored token regardless of liveness; `inspect`
  and `owner` are time-gated. The "agreement" relationship is
  asymmetric. Worth pinning: "at any sampled `now_ms`,
  `inspect(now_ms).map(|l| l.token)` agrees with `token(resource)`
  if `inspect` returned `Some`; otherwise no constraint between
  them."
- [A4] **In-transaction surface is silently excluded.** The
  spec-diff lists `claim`, `renew`, `release` (autocommit) but not
  `claim_in_tx`, `renew_in_tx`, `release_in_tx`. Both code paths
  exist in `bouncer-core` and have different lock-acquisition
  behavior. Either fold the `*_in_tx` variants into Phase 011 (and
  make sequences capable of opening a caller transaction) or
  declare them explicitly out of scope and slate them for 012.
- [A5] **Time-advancement model is undefined.** "Model time
  advancement" appears in the verb list but has no semantics
  attached: monotonic? per-step delta? bounded? May go backwards
  for read-only sampling? Decide in spec-diff before plan-time.
  Recommended: model clock is per-sequence monotonic; reads may
  sample at any `now_ms` including non-monotonic boundary samples;
  mutation operations advance the clock.
- [A6] **"Successful release clears live ownership but preserves
  token state" is a separate invariant from the others.** After a
  release the row exists with `owner = NULL`, `lease_expires_at_ms
  = NULL`, `token = N`. The next claim takes token N+1. The
  invariant should explicitly cover the row's *post-release shape*
  (owner cleared, expiry cleared, token preserved) so the runner
  catches drift from any of those three.

### Review verdict

The spec-diff captures the right idea but has five real
ambiguities that should be pinned before code lands ([A1], [A2],
[A3], [A5], [A6]) and one scope question ([A4]) and one
escape-clause gap ([N1]). [N2] is a small documentation pin. None
is blocking individually; collectively they add up to "tighten the
spec-diff before implementing."

## Plan Review 1

Target:
- plan review

Session:
- B

Model family:
- Claude Opus 4.7

Artifacts reviewed:
- `spec-diff.md`
- `plan.md`

Verification reviewed:
- existing `bouncer-core/src/lib.rs` size (1229 lines) to assess
  whether tests should land inline or in a separate file
- existing `bouncer-core` test layout
- prior phase build orders (007, 010) for closeout-pattern
  consistency

### Positive conformance review

- [P5] Plan's build order maps cleanly to spec-diff invariants:
  structs → RNG → generation → model → assertions → tests. Each
  step is concrete and reviewable.
- [P6] "Phase decisions" section pins the right things up front:
  no shared simulator crate, in-memory SQLite, explicit `now_ms`
  only, no concurrency, no large property-testing framework. These
  are exactly the scope-creep traps an invariant runner usually
  falls into.
- [P7] "Traps" section forbids two real anti-patterns: testing the
  model against itself, and wall-clock sleeps. Good explicit
  callouts.
- [P8] "Ambiguities noticed during planning" section honestly
  flags two real questions (non-monotonic generated `now_ms`,
  property-testing crate choice) instead of silently picking. The
  right behavior for a planner.
- [P9] "Files likely to change" includes closeout updates to
  CHANGELOG/SYSTEM/ROADMAP. Matches the rhythm 007 and 010
  established.

### Negative conformance review

- [N3] "Areas that should not be touched" is comprehensive, but
  "schema shape" and "production lease semantics" overlap with
  [N1] from the intent review: if the runner finds a real bug, the
  fix touches one or both. The plan needs the same escape-clause
  decision the spec-diff needs.
- [N4] Plan correctly excludes Python from this phase, but the
  Python binding currently exercises `bouncer-core` indirectly. If
  Phase 011 changes anything in core (even a test seam), Python
  tests should still pass. Worth one acceptance line: "Python
  tests still pass after this phase."

### Adversarial review

- [A7] **Test file location is unresolved.** Plan says "Add test
  module under `bouncer-core/src/lib.rs` *or* split test-only
  helpers if the file gets too large." `bouncer-core/src/lib.rs`
  is already 1229 lines, well over the 1000-line guideline that
  triggered the 010 split. The runner will add hundreds of lines.
  The answer is already "split" — pin it as
  `bouncer-core/tests/invariants.rs` (integration test, public API
  only) or `bouncer-core/src/tests_invariants.rs` (inline,
  matching the wrapper's split pattern). Recommended:
  `bouncer-core/tests/invariants.rs` so the runner exercises the
  public surface and cannot accidentally reach into private
  helpers.
- [A8] **RNG choice is deferred but should be picked.** Plan
  defers between "tiny deterministic RNG implemented in test code"
  and "already-present dev dependency." A 5-line xorshift64 in
  test code is the minimalist choice and adds zero dependencies.
  Recommended: pick xorshift64 explicitly so review and
  implementation stop debating.
- [A9] **Model-vs-SQLite assertion granularity is unspecified.**
  Plan says "assert model/SQLite agreement" but does not say *how*
  the runner reads SQLite. Two options: (1) through `core::inspect
  / owner / token` only — strong end-to-end check but cannot
  catch core APIs lying about state; (2) direct table reads on
  `bouncer_resources` — stronger, can catch core API drift, but
  couples the runner to schema. Recommended: assert through both
  layers — public API for the user-facing invariants, direct table
  reads for the row-shape invariants ([A6]).
- [A10] **Sequence-length and seed-count budgets are unspecified.**
  Plan says "bounded sequence length" and "many seeds" without
  numbers. CI cost depends on these. Recommended: 100 steps × 1000
  seeds for default `cargo test`; an `#[ignore]`-gated stress
  variant at 10K steps × 100K seeds for local soak.
- [A11] **Resource and owner cardinality unspecified.** "Several"
  is in the spec-diff. Recommended: 4 resources × 6 owners. Small
  enough that interesting collisions are likely; large enough that
  state space is non-trivial.
- [A12] **`*_in_tx` exclusion not in plan.** Plan omits the in-tx
  variants. If the spec-diff intent is to test core lease
  semantics, the in-tx path is part of that surface; if the intent
  is just autocommit, say so. Recommended: explicitly out of scope
  for 011, in scope for 012's settings matrix.
- [A13] **Model "mostly monotonic" is doing real work.** Plan
  flags this in "Ambiguities" but also bakes it into the model
  helpers. Pin one of: (a) sequence-monotonic mutation time + free
  read-time sampling; (b) per-step random delta from a monotonic
  base + free read-time sampling; (c) fully arbitrary mutation
  time across observations (then `now_ms` ordering becomes part of
  the test, not an assumption). Recommended: (a). Bouncer's
  contract treats the caller as a single timeline of writes;
  multi-timeline mutation interleaving is a Phase 012 concern.
- [A14] **Failure-mode for surfaced bugs is unresolved.** Plan
  says "if runner exposes ambiguity, stop and write into
  `reviews_and_decisions.md`." Good for ambiguities. But if the
  runner finds a real semantic bug in core, the plan also says "no
  production code changes unless review finds an unavoidable test
  seam." A bug fix is not a test seam. Pin the policy: bug fixes
  surfaced by 011 are in-scope and add to the phase's diff; if the
  bug is large, split into 011a (runner) + 011b (fix).
- [A15] **No mention of `prepare_cached` cost.** The runner will
  hammer `claim`/`renew`/`release` thousands of times. If
  `bouncer-core` doesn't already cache prepared statements, every
  iteration re-parses SQL. Worth a quick check before settling on
  budget defaults.

### Review verdict

The plan is sound in shape but defers seven real implementation
decisions ([A7]-[A13]) and leaves one policy question open
([A14]). All of them benefit from being pinned before
implementation rather than emerging from code. Recommend:

- pin file location, RNG choice, assertion-layer split, sequence
  budget, resource/owner cardinality, time model, and `*_in_tx`
  scope in `plan.md`
- pin escape-clause for surfaced bugs in `plan.md` and
  `spec-diff.md`
- close the [A1]-[A6] spec-diff ambiguities from Intent Review 1
  before writing the model code

After those pins, the plan is implementable as written.

## Review Response 1

Responding to:
- Intent Review 1
- Plan Review 1

Session:
- A

### Inputs

- [N1] escape clause for real bugs surfaced by runner
- [N2] SQLite configuration used by the runner
- [A1] observation-time scope for "at most one live owner"
- [A2] fresh first claim vs post-release/post-expiry reclaim token
- [A3] asymmetric `inspect` / `owner` / `token` agreement
- [A4] in-transaction surface scope
- [A5] time-advancement model
- [A6] post-release row-shape invariant
- [N3] bug-fix policy also needed in plan
- [N4] Python tests after production core changes
- [A7] test file location
- [A8] RNG choice
- [A9] assertion-layer split
- [A10] sequence budget
- [A11] resource/owner cardinality
- [A12] `*_in_tx` scope in plan
- [A13] time-model variant
- [A14] surfaced-bug policy
- [A15] SQL prepare/runtime cost

### Decisions

- [D1] Accept [A1].
  Action: `spec-diff.md` now scopes the one-live-owner invariant to
  every `now_ms` value sampled by the runner through `inspect`.
  Targets: `spec-diff.md`.

- [D2] Accept [A2].
  Action: `spec-diff.md` now distinguishes fresh-resource first claim
  (`token = 1`) from later successful claims after expiry or release
  (strictly larger token).
  Targets: `spec-diff.md`.

- [D3] Accept [A3].
  Action: `spec-diff.md` now says `inspect` and `owner` must agree at
  sampled times, while `token` is time-independent and only must match
  a live lease when `inspect` returns one.
  Targets: `spec-diff.md`.

- [D4] Accept [A4] and [A12].
  Action: `claim_in_tx`, `renew_in_tx`, and `release_in_tx` are
  explicitly out of scope for Phase 011 and move to Phase 012's SQLite
  behavior matrix.
  Targets: `spec-diff.md`, `plan.md`.

- [D5] Accept [A5] and [A13].
  Action: mutation operations use sequence-monotonic model time; read
  operations may sample non-monotonic times around lease boundaries.
  Targets: `spec-diff.md`, `plan.md`.

- [D6] Accept [A6] and [A9].
  Action: post-release row shape is now a named invariant. The plan now
  requires public API assertions plus direct table reads for row-shape
  invariants.
  Targets: `spec-diff.md`, `plan.md`.

- [D7] Accept [N1], [N3], and [A14].
  Action: small direct core bug fixes found by the runner are in scope;
  broad or ambiguous semantic fixes must split into a follow-up phase
  after a decision round.
  Targets: `spec-diff.md`, `plan.md`.

- [D8] Accept [N2].
  Action: the runner is pinned to in-memory SQLite with default pragmas.
  Targets: `spec-diff.md`, `plan.md`.

- [D9] Accept [N4].
  Action: `plan.md` now says Python tests still pass if any production
  core code changes.
  Targets: `plan.md`.

- [D10] Accept [A7].
  Action: implementation location is pinned to
  `bouncer-core/tests/invariants.rs`.
  Targets: `plan.md`.

- [D11] Accept [A8].
  Action: RNG choice is pinned to a tiny xorshift64-style deterministic
  RNG in test code, with no new property-testing dependency.
  Targets: `plan.md`.

- [D12] Accept [A10].
  Action: default generated budget is pinned at 1000 seeds × 100 steps;
  an ignored stress test is optional.
  Targets: `plan.md`.

- [D13] Accept [A11].
  Action: generator cardinality is pinned at 4 resources × 6 owners.
  Targets: `plan.md`.

- [D14] Accept [A15].
  Action: the traps section now calls out runtime/budget creep because
  repeated operations reprepare SQL today.
  Targets: `plan.md`.

### Verification

- Updated `spec-diff.md` and `plan.md` with explicit pins for every
  accepted finding.
- No code changed.
- No tests run; this is an artifact-planning response.

### Decision verdict

Phase 011 is now ready for implementation review handoff. The remaining
shape is intentionally narrow: core-only, deterministic, explicit-time,
autocommit operations, replayable by seed, with no Python or SQLite
settings matrix work.

## Implementation Notes 1

Session:
- A

Files changed:
- `bouncer-core/tests/invariants.rs` (new)

What landed:
- A single integration test file at the pinned location, exercising
  `bouncer-core`'s public API (`claim`, `renew`, `release`, `inspect`,
  `owner`, `token`) plus direct `bouncer_resources` reads for row-shape
  checks.
- A 5-line xorshift64-style RNG seeded from `u64`. No new
  property-testing dependency. Seed-zero collapse is avoided by mixing
  in a non-zero offset and forcing a non-zero state.
- An `Op` enum covering `Claim`, `Renew`, `Release`, `Inspect`, `Owner`,
  `Token`, and `AdvanceTime`.
- A `Model` with per-resource `last_token`, `row_owner`, and
  `row_expires_at_ms`. Mutators advance a sequence-monotonic clock by 1
  before applying their `now_ms`; `AdvanceTime` adds further deltas;
  reads do not move the clock. Read operations sample the clock plus
  expiry boundary points (exp-1, exp, exp+1).
- Per-step invariant checks across all 4 resources covering:
  - `inspect` agrees with `owner` at the model clock
  - `inspect` agrees with the model
  - `token()` agrees with the model and with a live lease's token
  - direct row read confirms `(owner, token, lease_expires_at_ms)`
    shape, including the post-release nullability invariant
- Two tests:
  - `fixed_sequence_exercises_full_lifecycle` — readable hand-written
    sequence covering first claim, busy, wrong-owner renew, valid
    renew, wrong-owner release, valid release, reclaim after release,
    expiry takeover, and token monotonicity through all of that.
  - `generated_invariants_hold_across_seeds` — 1000 seeds × 100 steps
    against in-memory SQLite with default pragmas.

Decisions honored:
- [D1]–[D14] all reflected in the runner.
- No production code changed. The runner did not surface a real core
  bug, so the in-scope-bug-fix clause ([D7]) was not exercised.
- Python was not touched.
- `*_in_tx` paths are not generated; they remain Phase 012's surface.

Verification:
- `cargo test -p bouncer-core` — 26 lib tests + 2 invariant tests pass.
  Generated test runs in ~8.7s on its own.
- `make test-rust` — entire Rust suite (core, wrapper, extension,
  invariants) passes. Generated test runs in ~21s under the combined
  invocation; acceptable for the pinned 1000 × 100 budget.
- Failure messages include `seed={seed} step={step}` in every assertion
  and panic so any future finding is replayable.

Notes for the reviewer:
- The runner depends only on `bouncer-core`'s public API and one direct
  `SELECT` from `bouncer_resources`. No private helpers reached.
- Generated test cost is dominated by per-step invariant queries (~16
  SQL calls per step). If budget creep becomes an issue later, the
  cheapest reduction is folding `inspect`/`owner`/`token` reads into a
  single `read_row`-derived check; left as-is for now since the public
  API is the contract and exercising it on every step is the point.

## Implementation Review 1

Target:
- implementation review

Session:
- B

Model family:
- GPT-5.5

Artifacts reviewed:
- `spec-diff.md`
- `plan.md`
- `reviews_and_decisions.md`
- `bouncer-core/tests/invariants.rs`

Verification reviewed:
- `cargo test -p bouncer-core`
- `make test-rust`

### Positive conformance review

- [P10] The runner lands at the pinned location:
  `bouncer-core/tests/invariants.rs`. It is an integration test using
  the public `bouncer-core` API rather than private helpers, matching
  [D10].
- [P11] The implementation honors the scope fences. It is core-only,
  in-memory SQLite with default pragmas, autocommit-only, explicit-time,
  and does not touch Python, the SQL extension surface, the Rust wrapper
  API, or production lease code.
- [P12] The generator matches the pinned shape: xorshift64-style RNG in
  test code, 4 resources, 6 owners, 1000 seeds, and 100 steps.
- [P13] The assertion split matches [D6]. User-facing behavior is checked
  through `claim`, `renew`, `release`, `inspect`, `owner`, and `token`;
  direct `bouncer_resources` reads are used only for row-shape
  invariants.
- [P14] The fixed sequence is readable and covers the promised lifecycle:
  first claim, busy claim, wrong-owner renew, valid renew,
  wrong-owner release, valid release, reclaim after release, expiry
  takeover, and token checks.
- [P15] Failure messages include `seed={seed}` and `step={step}` on every
  panic/assertion path I checked. That satisfies the replayability bar
  in the spec diff.
- [P16] Verification passed locally: `cargo test -p bouncer-core` passed
  26 lib tests plus 2 invariant tests, and `make test-rust` passed the
  Rust wrapper, extension-load, core, and invariant suites.

### Negative conformance review

- [N5] The phase contract says "successful renew keeps the token and
  extends expiry," but the runner models and accepts the current core
  behavior: renew sets `lease_expires_at_ms = now_ms + ttl_ms`, which
  can shorten a lease if the caller renews with a smaller TTL than the
  remaining lease duration. In `bouncer-core/tests/invariants.rs`, the
  renew branch checks equality with `now_ms + ttl_ms`, not extension
  relative to the previous expiry. This is not necessarily a code bug;
  it is a semantic wording mismatch that should be decided before
  closeout. Either update the Phase 011 spec/SYSTEM wording to say
  "refreshes expiry to now + ttl" or change the generator so successful
  renews always use a TTL that truly extends and assert that invariant.

### Adversarial review

- [A16] The runner is valuable but still model-coupled to the current
  core semantics. That is fine for Phase 011, but [N5] shows the main
  risk: if the model encodes existing behavior where the spec says a
  stronger property, the generated test will bless drift instead of
  catching it. The right mitigation is not a larger runner; it is
  sharper semantic wording for operations like renew before future
  phases build on this harness.
- [A17] Replay is technically possible from seed and step, but not yet
  ergonomic. A future failure still requires re-running the whole seed
  sequence or editing the test locally. Acceptable for V1, but if this
  runner catches a real bug, the next hardening pass should consider a
  small single-seed/single-limit helper or trace dump before increasing
  budgets.

### Review verdict

Accepted with one semantic follow-up before Phase 011 closeout: resolve
[N5]. The implementation itself is solid, scoped, and tested. I would
not block on [A17]; it is a future ergonomics improvement, not a Phase
011 requirement.

## Implementation Response 1

Responding to:
- Implementation Review 1

Session:
- A

### Inputs

- [N5] Renew can shorten a lease under the current implementation even
  though the Phase 011 contract says renew extends expiry.
- [A16] The runner can bless existing behavior if the model encodes a
  weaker semantic property than the spec intends.
- [A17] Replay ergonomics can improve later.

### Decisions

- [D15] Accept [N5] as a real core semantic bug surfaced by Phase 011.
  Action: `renew_in_tx` now uses
  `max(current_expiry, now_ms + ttl_ms)`, so renew is
  extend-or-preserve and never shortens a live lease.
  Targets: `bouncer-core/src/lib.rs`,
  `bouncer-core/tests/invariants.rs`, `spec-diff.md`.

- [D16] Accept [A16].
  Action: the invariant model now asserts the stronger renew contract
  instead of mirroring the old `now_ms + ttl_ms` replacement behavior.
  Targets: `bouncer-core/tests/invariants.rs`.

- [D17] Defer [A17].
  Action: seed/step replay remains sufficient for Phase 011. Trace dump
  or single-seed helpers can be considered if the runner catches a
  difficult future bug.
  Targets: future hardening only.

### Verification

- Added a direct core regression test:
  `renew_does_not_shorten_existing_lease`.
- Updated the Phase 011 spec wording:
  successful renew keeps the token and never shortens expiry; expiry
  becomes `max(current_expiry, now_ms + ttl_ms)`.
- `cargo test -p bouncer-core` — 27 lib tests + 2 invariant tests pass.
- `make test-rust` — Rust wrapper, extension-load, core, and invariant
  suites pass.
- `make test` — full Rust suite, extension build, Python editable
  build, and 20 Python tests pass.

### Decision verdict

[N5] is resolved by making renew semantics stronger rather than
weakening the spec wording. Bouncer V1 does not support shortening a
live lease via `renew`; callers can `release` for immediate handoff, and
any future shorten/yield behavior should be an explicit separate API.

## Implementation Notes 2

Session:
- A

Files changed:
- `bouncer-core/src/lib.rs`
- `bouncer-core/tests/invariants.rs`
- `spec-diff.md`

What landed:
- `renew_in_tx` now preserves or extends a live lease expiry instead of
  blindly replacing it with `now_ms + ttl_ms`.
- The new contract is:
  `lease_expires_at_ms = max(current_expiry, now_ms + ttl_ms)`.
- A direct core regression test now proves renew does not shorten an
  existing live lease.
- The deterministic invariant runner now models and asserts the stronger
  renew behavior rather than mirroring the earlier weaker semantics.

Correction to Implementation Notes 1:
- Production code did change after review. `[D15]` intentionally landed
  a small direct core fix inside Phase 011 under the already-accepted
  bug-fix policy.

Verification:
- `cargo test -p bouncer-core` passes with 27 core lib tests and 2
  invariant tests.
- `make test-rust` passes.
- `make test` passes, including the Python binding tests.
