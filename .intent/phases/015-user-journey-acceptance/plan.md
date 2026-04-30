# Plan

Phase: 015 — user journey acceptance

Session:
- A

## What exactly we are building

We are building a small public-surface acceptance suite.

The point is to prove that a user can do the normal things they would
expect to do with Bouncer and get the right visible result:

1. bootstrap a fresh file
2. take the first lease
3. have a second caller see busy
4. release and reclaim with token increase
5. expire and reclaim with token increase
6. mix surfaces on one file and still see the same lease state
7. use a caller-owned transaction and get one atomic visible result
8. hit drifted schema and get a loud bootstrap failure

## What explicitly will not change

- We are not changing lease semantics.
- We are not changing schema rules.
- We are not adding bindings.
- We are not adding migration behavior.
- We are not expanding pragma work.
- We are not claiming downstream stale-actor protection is end-to-end
  proved outside Bouncer.

## How the proof will work

Use three layers on purpose:

### 1. Unit / lower-level supporting proof

Keep these green:

- `bouncer-core/tests/invariants.rs`
- `bouncer-core/tests/integrity.rs`
- `bouncer-core/tests/sqlite_matrix.rs`
- `bouncer-core/tests/pragma_matrix.rs`
- existing wrapper / Python integration tests

These support the phase but do not close the new claims.

### 2. Integration proof

Keep the existing wrapper / SQL / Python cross-surface integration paths
green so the shipped surfaces still share one contract.

### 3. Direct user-shaped acceptance proof

Add one acceptance file:

- `packages/bouncer/tests/user_journeys.rs`

Add Python acceptance coverage too:

- `packages/bouncer-py/tests/test_bouncer.py`

Python is in scope in this phase. It is not optional.

## Exact journeys and exact direct proof

1. **Fresh bootstrap + first claim**
   - direct proof:
     - wrapper bootstrap on a fresh file
     - wrapper claim succeeds

2. **Independent second caller sees busy**
   - direct proof:
     - one live caller claims through wrapper
     - a separate live caller on a separate connection sees busy

3. **Release then reclaim increments token**
   - direct proof:
     - first caller claims
     - first caller releases
     - second caller reclaims
     - visible token is larger

4. **Expiry then reclaim increments token**
   - direct proof:
     - use the SQL extension for this journey so time is explicit and
       deterministic
     - first claim at `now_ms=t1`
     - reclaim at `now_ms > expiry`
     - visible token is larger

5. **Cross-surface interoperability on one file**
   - direct proof:
     - Python claims
     - SQL or Rust sees the live lease
     - SQL or Rust releases or reclaims
     - Python sees the new visible state

6. **Caller-owned transaction gives one atomic visible result**
   - direct proof:
     - caller opens a transaction through a public surface
     - performs one business write plus one lease mutation
     - a fresh connection does not see partial state before commit
     - a fresh connection sees both after commit

7. **Drifted schema fails loudly through public bootstrap**
   - direct proof:
     - wrapper `Bouncer::bootstrap()`
     - Python `bootstrap()`
     - SQL `bouncer_bootstrap()`
   - each should fail loudly on the same drifted file

## Implementation notes

- Use file-backed databases only.
- Use separate live connections whenever the claim is about independent
  callers.
- Do not use sleeps for expiry. Use the SQL surface's explicit `now_ms`
  to keep the expiry journey deterministic.
- Keep assertions user-visible first:
  - success versus busy
  - visible owner / token / reclaim result
  - visible bootstrap failure
- Only inspect raw rows if a visible public result cannot express the
  needed confirmation.

## Acceptance

The phase is done when:

- every journey above has a named direct-proof test
- Python really is part of the interop proof
- expiry/reclaim is deterministic and not sleep-based
- caller-owned transaction behavior is proven through one concrete
  user-shaped journey
- drifted-schema bootstrap is proven through all three public bootstrap
  surfaces
- lower-level suites are still green

## Tests and evidence

- `cargo test -p bouncer --test user_journeys`
- `cargo test -p bouncer-core -p bouncer`
- `make test-rust`
- `make test`

## Files likely to change

- `packages/bouncer/tests/user_journeys.rs`
- `packages/bouncer-py/tests/test_bouncer.py`
- `.intent/phases/015-user-journey-acceptance/*`
