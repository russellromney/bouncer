# Plan

Phase: 011 — deterministic invariant runner

Session:
- A

## Goal

Harden `bouncer-core` by adding a deterministic, replayable invariant
runner for the lease state machine. The point is to positively search
for ways Bouncer could violate its core promises before we move on to
SQLite settings, corruption cases, or more bindings.

## Context

Bouncer's core semantics are intentionally small: one resource, one live
owner, expiry, release, renew, and monotonic fencing tokens. Existing
tests cover many named scenarios. Phase 011 adds generated sequences so
we can exercise combinations that humans are unlikely to write by hand.

Python is not part of this phase. It remains an example binding. The
primary correctness surface here is `bouncer-core`.

## References

- `SYSTEM.md`
- `ROADMAP.md`
- `.intent/phases/011-deterministic-invariant-runner/spec-diff.md`
- `bouncer-core/src/lib.rs`

## Mapping from spec diff to implementation

- Seeded operation generation maps to a new test-only runner in
  `bouncer-core`.
- Explicit-time operations map directly to existing core functions:
  `claim`, `renew`, `release`, `inspect`, `owner`, and `token`.
- Invariants map to assertions run after every generated operation.
- Replayability maps to failure messages that include seed, step index,
  and the operation trace or enough recent trace to reproduce.

## Phase decisions

- Keep the runner inside `bouncer-core` tests for now. Do not create a
  shared simulator crate yet.
- Use in-memory SQLite for the first runner unless implementation finds
  a reason that file-backed state is necessary for a specific invariant.
- Use explicit `now_ms` only. Do not read system time.
- Do not add random OS threads or concurrency in this phase. SQLite
  contention is Phase 012.
- Prefer a tiny deterministic RNG implemented in test code or an
  already-present dev dependency. Do not add a large property-testing
  framework unless it clearly improves replayability and keeps the code
  understandable.
- Treat generated checks as additional proof, not a replacement for the
  named scenario tests.

## Proposed implementation approach

Add a test module under `bouncer-core/src/lib.rs` or split test-only
helpers if the file gets too large. The runner can be simple:

1. Define an `Op` enum for generated operations.
2. Define a small deterministic RNG from `u64` seed to choices.
3. Define a model state per resource with last known token, optional
   live lease, and the current model time.
4. Generate a bounded sequence for each seed.
5. Apply each operation to SQLite through `bouncer-core`.
6. Apply the expected semantic effect to the model.
7. After every step, assert model/SQLite agreement.

The model does not need to replicate every SQL detail. It only needs to
encode Bouncer's intended lease semantics clearly enough to catch drift:
who should be live at `now_ms`, what token should be visible, and which
operations should mutate or not mutate.

## Build order

1. Add test-only operation and model structs.
2. Add a deterministic RNG helper and fixed seed list.
3. Implement operation generation across several resource names and
   owner names.
4. Implement model update helpers for claim/renew/release/time.
5. Implement invariant assertions after each operation.
6. Add one named fixed-sequence test that is easy to read.
7. Add one generated test over many seeds and bounded sequence length.
8. Run core tests, then full Rust tests if practical.

## Acceptance

- Generated tests cover at least:
  - multiple resources
  - multiple owners
  - successful first claim
  - busy claim
  - wrong-owner renew
  - successful renew
  - wrong-owner release
  - successful release
  - expiry takeover
  - reclaim after release
- Failures identify the seed and step.
- Tests are deterministic across runs.
- No production code changes unless review finds an unavoidable test seam
  that belongs in production.

## Tests and evidence

- `cargo test -p bouncer-core`
- `make test-rust` if available and not prohibitively slow
- `make test` if the environment already has the Python build artifacts
  needed for the full suite

## Traps

- Do not accidentally test the model against itself. Every invariant
  must observe SQLite through core APIs or direct table reads when
  appropriate.
- Do not let generated time move only forward if a useful invariant
  should hold for arbitrary explicit `now_ms` reads. It is fine for the
  model's main clock to advance, but `inspect/owner` should also sample
  around lease boundaries.
- Do not use wall-clock sleeps.
- Do not fold in SQLite lock contention. That needs a matrix and clearer
  failure taxonomy in Phase 012.
- Do not make failures impossible to reproduce by hiding the seed or
  relying on global randomness.

## Files likely to change

- `bouncer-core/src/lib.rs`
- possibly a new `bouncer-core/src/tests_invariants.rs` if splitting the
  test module keeps files easier to review
- `.intent/phases/011-deterministic-invariant-runner/*`
- `CHANGELOG.md` at closeout
- `SYSTEM.md` at closeout only if the runner lands and becomes part of
  the proved baseline
- `ROADMAP.md` at closeout

## Areas that should not be touched

- SQL extension function names or behavior
- Rust wrapper public API
- Python binding code or tests
- schema shape
- package publishing or binding strategy

## Assumptions and risks

- The runner may expose an ambiguity in current semantics. If that
  happens, stop and write the ambiguity into
  `reviews_and_decisions.md` rather than silently choosing behavior in
  code.
- A too-clever generator will be worse than a smaller readable one.
  The first version should privilege clear invariants and replay.
- If the generated tests are slow, reduce seeds/steps for default CI and
  leave a heavier ignored test or documented command for local stress.

## Commands

- `cargo test -p bouncer-core`
- `make test-rust`
- `make test`

## Ambiguities noticed during planning

- Whether to allow non-monotonic generated `now_ms` for mutation
  operations. Initial recommendation: keep mutation time mostly
  monotonic per sequence, but sample reads around boundaries. If review
  wants fully arbitrary mutation times, pin the expected semantics first.
- Whether to use a property-testing crate. Initial recommendation:
  avoid it for V1 unless review strongly prefers `proptest` for
  shrinking.
