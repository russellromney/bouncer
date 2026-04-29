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
  - at most one live owner exists per resource at any observed time
  - fencing tokens never decrease for a resource
  - successful first claim initializes token `1`
  - successful reclaim after expiry or release increments token
  - busy claims do not mutate state
  - wrong-owner renew/release attempts do not mutate state
  - successful renew keeps the token and extends expiry
  - successful release clears live ownership but preserves token state
  - expired leases are not live and can be taken over
  - `inspect`, `owner`, and `token` agree with each other
- Failing generated cases must print enough information to replay the
  seed and operation index.

## What does not change

- No production lease semantics change.
- No schema change.
- No SQL extension behavior change.
- No Rust wrapper public API change.
- No Python binding change.
- No SQLite settings matrix, lock-contention matrix, VFS shim, or fault
  injection in this phase.
- No shared Honker-family simulator crate yet.

## How we will verify it

- Add core-level deterministic invariant tests that run many generated
  operation sequences against an in-memory SQLite database.
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
