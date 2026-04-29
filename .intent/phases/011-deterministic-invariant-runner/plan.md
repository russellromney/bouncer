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
- Put the runner in `bouncer-core/tests/invariants.rs`, not inline in
  `bouncer-core/src/lib.rs`. The core file is already large, and an
  integration test keeps the runner honest by using the public API.
- Use in-memory SQLite with default pragmas for this phase.
- Use explicit `now_ms` only. Do not read system time.
- Do not add random OS threads or concurrency in this phase. SQLite
  contention is Phase 012.
- Exclude `claim_in_tx`, `renew_in_tx`, and `release_in_tx` from this
  phase. Caller-owned transaction behavior belongs in Phase 012's
  SQLite behavior matrix.
- Use a tiny xorshift64-style deterministic RNG implemented in test
  code. Do not add a property-testing dependency for V1.
- Use 4 resource names and 6 owner names.
- Run 100 steps across 1000 seeds in the default generated test.
  Optionally add an ignored stress test with larger budgets if the
  implementation stays cheap.
- Use sequence-monotonic mutation time. Allow read operations to sample
  non-monotonic times around lease boundaries.
- Assert through both layers: public API calls for user-facing
  invariants, direct table reads for row-shape invariants such as
  post-release `owner = NULL`, `lease_expires_at_ms = NULL`, and token
  preservation.
- Treat generated checks as additional proof, not a replacement for the
  named scenario tests.
- If the runner finds a real core bug, a small direct fix is in scope
  for this phase. If the fix is broad or changes intended semantics,
  split runner and fix into separate phases after a decision round.

## Proposed implementation approach

Add an integration test at `bouncer-core/tests/invariants.rs`. The
runner can be simple:

1. Define an `Op` enum for generated operations.
2. Define a small deterministic RNG from `u64` seed to choices.
3. Define a model state per resource with last known token, optional
   live lease, and the current model time.
4. Generate a bounded sequence for each seed: 100 steps across 1000
   seeds in the default test.
5. Apply each operation to SQLite through `bouncer-core`.
6. Apply the expected semantic effect to the model.
7. After every step, assert model/SQLite agreement through public API
   reads and, where row shape matters, direct `bouncer_resources` reads.

The model does not need to replicate every SQL detail. It only needs to
encode Bouncer's intended lease semantics clearly enough to catch drift:
who should be live at `now_ms`, what token should be visible, and which
operations should mutate or not mutate.

## Build order

1. Add `bouncer-core/tests/invariants.rs`.
2. Add test-only operation and model structs.
3. Add a xorshift64 deterministic RNG helper and fixed seed list.
4. Implement operation generation across 4 resource names and 6 owner
   names.
5. Implement model update helpers for claim/renew/release/time.
6. Implement invariant assertions after each operation.
7. Add one named fixed-sequence test that is easy to read.
8. Add one generated test over 1000 seeds × 100 steps.
9. Optionally add an ignored stress test if it remains cheap and useful.
10. Run core tests, then full Rust tests if practical.

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
- Default generated test budget is pinned at 1000 seeds × 100 steps.
- Runner uses public core APIs for lease behavior and direct table reads
  only for row-shape checks.
- Python tests still pass if any production core code changes.
- Production code changes happen only for a real core bug found by the
  runner; broad or ambiguous fixes split into a follow-up phase.

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
- Do not fold in caller-owned transaction generation. The `*_in_tx`
  path joins the SQLite behavior matrix in Phase 012.
- Do not make failures impossible to reproduce by hiding the seed or
  relying on global randomness.
- Check runtime before increasing budgets; repeated `claim`/`renew`/
  `release` calls reprepare SQL today, so budget creep can get noisy.

## Files likely to change

- `bouncer-core/tests/invariants.rs`
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
- The runner may expose a real implementation bug. Small direct fixes
  are in scope; large or semantic fixes split into a follow-up phase.
- A too-clever generator will be worse than a smaller readable one.
  The first version should privilege clear invariants and replay.
- If the generated tests are slow, reduce seeds/steps for default CI and
  leave a heavier ignored test or documented command for local stress.

## Commands

- `cargo test -p bouncer-core`
- `make test-rust`
- `make test`

## Ambiguities noticed during planning

- No open planning ambiguities remain after Plan Review 1 response.
