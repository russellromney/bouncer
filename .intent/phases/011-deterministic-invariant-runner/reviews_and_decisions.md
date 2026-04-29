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
