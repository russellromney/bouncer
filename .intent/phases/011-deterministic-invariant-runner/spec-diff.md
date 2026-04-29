# Spec Diff

Phase: 011 — deterministic invariant runner

Session:
- A

## What changes

- Bouncer gains a deterministic Rust test runner for `bouncer-core`
  lease operations.
- The runner generates seeded sequences of explicit-time operations
  over multiple resources and owners:
  - `claim(resource, owner, now_ms, ttl_ms)`
  - `renew(resource, owner, now_ms, ttl_ms)`
  - `release(resource, owner, now_ms)`
  - `inspect(resource, now_ms)`
  - `owner(resource, now_ms)`
  - `token(resource)`
  - time advancement in the model
- The runner checks lease invariants after every generated operation:
  - at every `now_ms` value sampled by the runner, `inspect(resource,
    now_ms)` returns at most one live owner for that resource
  - fencing tokens never decrease for a resource
  - the first successful claim against a fresh resource initializes
    token `1`
  - every later successful claim after expiry or release produces a
    strictly larger token
  - busy claims do not mutate state
  - wrong-owner renew/release attempts do not mutate state
  - successful renew keeps the token and extends expiry
  - successful release clears live ownership, clears expiry, preserves
    token state, and leaves the row reclaimable
  - expired leases are not live and can be taken over
  - `inspect` and `owner` agree at every sampled `now_ms`
  - if `inspect(resource, now_ms)` returns a live lease, that lease's
    token agrees with `token(resource)`; if `inspect` returns no live
    lease, `token(resource)` may still return the resource's last stored
    fencing token
- Failing generated cases must print enough information to replay the
  seed and operation index.
- Mutation operations use a monotonic per-sequence model clock. Read
  operations may sample non-monotonic times around lease boundaries.

## What does not change

- No production lease semantics change unless the runner exposes a real
  core bug. Small direct bug fixes are in scope for this phase; broad or
  ambiguous semantic changes must split into a follow-up phase after a
  decision round.
- No schema change.
- No SQL extension behavior change.
- No Rust wrapper public API change.
- No Python binding change.
- No SQLite settings matrix, lock-contention matrix, VFS shim, or fault
  injection in this phase.
- No `claim_in_tx`, `renew_in_tx`, or `release_in_tx` generation in this
  phase. Caller-owned transaction behavior belongs in the SQLite
  behavior matrix phase.
- No shared Honker-family simulator crate yet.

## How we will verify it

- Add core-level deterministic invariant tests that run many generated
  operation sequences against an in-memory SQLite database with default
  pragmas.
- Add at least one hand-written seeded regression test or fixed-seed
  test that exercises claim, busy, expiry takeover, renew, release, and
  reclaim in one sequence.
- `cargo test -p bouncer-core` passes.
- `make test-rust` or `make test` passes if available in the local
  environment.

## Notes

- This is the smallest useful DST-shaped step. It should be boring,
  local, replayable, and easy to delete or extract later.
- The runner should prefer explicit code over clever property-testing
  framework magic unless a dependency materially improves replay and
  shrinking without obscuring the invariants.
