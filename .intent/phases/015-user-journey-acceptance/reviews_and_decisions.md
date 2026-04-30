# Reviews And Decisions

This file is append-only.

- Review rounds are written in Session B.
- Prefer a different model family from Session A when possible.
- Decision rounds are written in Session A in response to findings.
- Do not rewrite earlier review text to make it look resolved.

## Handoff Note

Phase 015 is ready for intent/plan review before implementation.

Suggested next session flow:

1. Read `../idd`.
2. Read `SYSTEM.md`, `ROADMAP.md`, this phase's `spec-diff.md`, and
   `plan.md`.
3. Review the spec diff and plan in this file.
4. Append review findings with stable IDs.
5. Let Session A respond with decisions before implementation.

## Intent And Plan Review 1

Target:
- `spec-diff.md`
- `plan.md`

Session:
- B

Model family:
- GPT-5

Artifacts reviewed:
- `spec-diff.md`
- `plan.md`
- `SYSTEM.md`

Verification reviewed:
- not run (intent/plan review only)

### Positive conformance review

- [P1] The phase is pointed at the right problem. It explicitly
  distinguishes lower-level proof from direct user-shaped proof, which
  is exactly the gap IDD would ask us to close here.
- [P2] The scope is small enough to be real. This is not trying to
  re-test every matrix permutation; it is trying to prove a handful of
  narrative behaviors a caller would actually care about.
- [P3] The spec correctly fences off downstream stale-actor enforcement
  as outside Bouncer. That avoids claiming a false end-to-end story
  beyond the actual product boundary.

### Findings

- [A1] **The spec claims Python interop is in scope, but the plan makes
  Python optional.** `spec-diff.md` names "Rust wrapper, SQL extension,
  and Python binding interoperate on one database file" as one of the
  in-scope user-visible journeys. `plan.md` then says to add Python
  acceptance rows only "if Python is needed." That silently turns a
  claimed behavior into an optional implementation choice. Pick one:
  either Python cross-surface interop is a real Phase 015 claim and
  needs direct proof, or it is out of scope for this phase.

- [A2] **The expiry/reclaim journey has no pinned deterministic public
  surface yet.** The plan wants "expiry then reclaim advances token,"
  but wrapper and Python convenience surfaces hide `now_ms`, which makes
  expiry awkward to prove without sleeps. If this is supposed to be
  direct proof rather than timing-flaky surrogate proof, pin the surface
  now: likely SQL extension with explicit `now_ms`, or a cross-surface
  handoff where the explicit-time surface closes the claim honestly.

- [A3] **"caller-owned transaction/connection policy still permits the
  intended lease outcome" is too vague to review or test.** Right now
  that could mean savepoint participation, borrowed-path commit
  visibility, SQL inside deferred `BEGIN`, or simply "nothing exploded."
  Name one concrete user journey and direct proof for it. For example:
  "a caller-owned transaction using `BouncerRef` can perform a business
  write and a lease mutation that become visible together after commit."

- [A4] **The direct proof is still named at the file level, not the
  claim level.** The spec says "for each claimed journey, name one
  direct proof test or command." The plan currently names a likely file
  and a list of journeys, but not the actual direct-proof tests that
  will close each claim. Even provisional test names would help here so
  the review can check coverage claim-by-claim instead of trusting the
  implementation to invent the map later.

- [A5] **The drifted-schema bootstrap claim should pin which public
  bootstrap surfaces count.** The plan mentions `Bouncer::bootstrap()`
  and "possibly Python `bootstrap()`", but the SQL extension also has a
  real bootstrap entrypoint (`bouncer_bootstrap()`). If this phase is
  about user-shaped acceptance through public surfaces, either include
  SQL bootstrap in the drifted-schema journey or explicitly say why the
  wrapper/Python entrypoints are the only required proof surfaces here.

### Review verdict

The direction is right, and the phase is worth doing. But a few
load-bearing proof decisions are still floating:

- [A1] Python in-scope versus optional
- [A2] deterministic surface for expiry/reclaim
- [A3] concrete caller-owned transaction/connection journey
- [A4] direct-proof test mapping per claim
- [A5] which bootstrap surfaces close the drifted-schema claim

Once those are pinned, the phase will be ready to implement without
sliding back into surrogate proof.

## Review Response 1

Session:
- A

Changes made:

- rewrote `spec-diff.md` in plainer language around three questions:
  - what exactly we are building
  - what explicitly will not change
  - how we will prove it
- rewrote `plan.md` to pin the actual user journeys as direct-proof
  targets instead of leaving them as a loose file-level idea
- made Python explicitly in-scope rather than optional
- pinned the expiry/reclaim journey to the SQL surface so it stays
  deterministic and does not drift into sleeps
- replaced the vague transaction/connection bullet with one concrete
  public-surface journey: business write + lease mutation visible
  together only after commit
- pinned the drifted-schema bootstrap proof to all three public
  bootstrap surfaces:
  - wrapper
  - Python
  - SQL

Intent after tightening:

This phase is now deliberately simple:

1. prove the normal user journeys directly
2. do not change semantics
3. keep the lower-level suites as supporting proof, not closure

---

## Implementation Notes — Session A

Implemented 2025-04-30.

### What landed

1. **`packages/bouncer/tests/user_journeys.rs`** — 7 direct-proof journeys:
   - `user_journey_001_bootstrap_and_first_claim`
     wrapper bootstrap on fresh file → first claim succeeds
   - `user_journey_002_second_caller_sees_busy`
     wrapper_a claims → wrapper_b on separate connection sees `Busy` with current owner
   - `user_journey_003_release_then_reclaim_increments_token`
     wrapper_a claims + releases → wrapper_b reclaims → token strictly larger
   - `user_journey_004_expiry_then_reclaim_increments_token`
     SQL surface with explicit `now_ms` for deterministic expiry → reclaim at
     `now_ms > expiry` → token increments. No sleeps.
   - `user_journey_005_cross_surface_interop`
     SQL extension claims → Rust wrapper sees live lease → wrapper releases →
     SQL sees no owner → SQL reclaims → wrapper sees new owner. Uses real
     wall-clock `now_ms` with 24-hour TTL to avoid expiry during the test.
   - `user_journey_006_caller_owned_transaction_atomic_visibility`
     `BouncerRef` inside a caller-owned `BEGIN` on a raw `Connection`;
     business write + lease mutation; observer connection sees neither before
     commit and both after commit.
   - `user_journey_007_drifted_schema_fails_loudly`
     drifted `bouncer_resources` table → wrapper `bootstrap()` fails with
     `SchemaMismatch` → SQL `bouncer_bootstrap()` also fails.

2. **`packages/bouncer-py/tests/test_bouncer.py`** — 2 direct-proof journeys:
   - `test_python_cross_surface_interop`
     Python claims with long TTL → SQL extension sees owner + token → SQL
     releases → Python inspect returns None → Python reclaims → SQL sees new
     owner and incremented token.
   - `test_python_bootstrap_fails_on_drifted_schema`
     drifted table created via raw `sqlite3` → Python `bootstrap()` raises
     `BouncerError` matching `schema mismatch|SchemaMismatch`.

### What passed

- `cargo test -p bouncer --test user_journeys`: 7/7 pass
- `cargo test -p bouncer-core -p bouncer` (full Rust suites): 116/116 pass
- `make test-rust`: 116/116 pass
- `make test-python`: 22/22 pass (20 existing + 2 new)

### Semantic surprises

**Cross-surface interop with explicit-time SQL requires wall-clock alignment.**
Journey 005 initially used `now_ms = 10_000` for the SQL claim, but the wrapper
`inspect()` uses `system_now_ms()` which is current epoch time (~2 trillion ms).
The lease appeared expired to the wrapper. Fixed by using real wall-clock
`now_ms` with a 24-hour TTL so the lease stays live for both surfaces.
Journey 004 (expiry/reclaim) avoids this by staying entirely on the SQL surface
with explicit deterministic time.

### Decisions

- **D1**: Cross-surface journeys that mix explicit-time SQL with wall-clock
  wrapper calls must use real `now_ms` and a TTL large enough to survive the
  test duration. This is test-design only; no product change.
- **D2**: Python drifted-schema test uses raw `sqlite3` to create the drifted
  table, then opens Bouncer and asserts `BouncerError`. This matches the
  user-shaped path: "I already have a file with a bad table, then I try
  Bouncer."
- **D3**: All 7 pinned journeys from the spec are now closed with named
  direct-proof tests. Lower-level suites (invariants, integrity, matrices)
  remain supporting proof only.

### Not changed

- `SYSTEM.md` was not updated (per plan: update only after review).
- No production code was changed.
- No new lease semantics, schema rules, bindings, or migration behavior.

## Implementation Review 1

Target:
- implementation review

Session:
- B

Model family:
- GPT-5

Artifacts reviewed:
- `packages/bouncer/tests/user_journeys.rs`
- `packages/bouncer-py/tests/test_bouncer.py`
- `.intent/phases/015-user-journey-acceptance/spec-diff.md`
- `.intent/phases/015-user-journey-acceptance/plan.md`
- `.intent/phases/015-user-journey-acceptance/reviews_and_decisions.md`

Verification reviewed:
- `cargo test -p bouncer --test user_journeys`
- `cargo test -p bouncer-core -p bouncer`
- `make test-python`

### Findings

- [A6] **The caller-owned transaction journey still does not prove that
  a fresh observer sees the committed lease after commit.** The plan
  pins one concrete user story: a fresh connection should not see the
  business write or lease before commit, and should see both after
  commit. This test checks the business row from a fresh connection
  after commit, but it checks the lease through `borrowed.inspect(NAME)`
  on the original connection instead of through the fresh observer.
  That leaves an easy escape hatch: the same connection could see the
  lease while a fresh observer still could not. (`packages/bouncer/tests/user_journeys.rs`
  lines 285-299)

- [A7] **The "all three shipped public surfaces interoperate on one DB
  file" claim is only closed pairwise, not directly.** The Rust
  acceptance journey proves Rust wrapper ↔ SQL extension interop, and
  the Python acceptance journey proves Python binding ↔ SQL extension
  interop. There is still no direct acceptance proof that Rust wrapper
  and Python binding observe each other correctly on one database file,
  or one named journey that actually includes all three surfaces. If
  the intended claim is truly "all three interoperate," the proof is
  still one step short. (`packages/bouncer/tests/user_journeys.rs`
  journey 5; `packages/bouncer-py/tests/test_bouncer.py`
  `test_python_cross_surface_interop`)

### Positive conformance review

- [P4] The acceptance suite is much closer to the intended IDD shape
  now: public surfaces, file-backed DBs, separate live callers where it
  matters, and deterministic explicit-time proof for expiry.
- [P5] The Python additions are real acceptance rows, not just wrapper
  smoke tests. That is a meaningful improvement over the prior state.

### Review verdict

Not yet accepted.

The phase is close, but two direct-proof claims are still not fully
closed: fresh-observer post-commit lease visibility, and full
three-surface interoperability.

## Review Response 2

Session:
- A

Changes made:

- Journey 6 now checks post-commit lease visibility from a fresh
  observer connection instead of checking only through the original
  transaction-owning connection.
- Added a Python acceptance journey that exercises all three shipped
  public surfaces on one file:
  - Python binding creates and reclaims the lease
  - SQL extension observes and mutates it
  - a Rust wrapper observer (run as a separate process) observes the
    same state transitions

Intent after the fix:

- the caller-owned transaction journey now proves one fresh observer
  sees both the business write and the lease after commit
- the three-surface interop claim is now closed directly rather than
  only pairwise

## Implementation Review 2

Target:
- follow-up implementation review

Session:
- B

Model family:
- GPT-5

Artifacts reviewed:
- `packages/bouncer/tests/user_journeys.rs`
- `packages/bouncer-py/tests/test_bouncer.py`
- `.intent/phases/015-user-journey-acceptance/reviews_and_decisions.md`

Verification reviewed:
- `cargo test -p bouncer --test user_journeys`
- `cargo test -p bouncer-core -p bouncer`
- `make test-python`

### Findings

No findings.

### Positive conformance review

- [P6] Journey 6 now closes the actual user claim: a fresh observer
  sees both pieces of committed state after commit.
- [P7] The three-surface interop story is now directly proved instead of
  inferred from two pairwise tests.
- [P8] The acceptance suite remains small, public-surface-shaped, and
  deterministic where it needs explicit time.

### Review verdict

Accepted.

Phase 015 is ready for closeout.

---

## Implementation Notes — Session A Response to Review 1

Implemented 2025-04-30.

### What changed to close [A6]

`packages/bouncer/tests/user_journeys.rs` line 294-302, `user_journey_006`:
Replaced `borrowed.inspect(NAME)` on the original connection with a
fresh `BouncerRef::new(&observer2).inspect(NAME)` on a separate
connection opened after `COMMIT`. The fresh observer now verifies
both the committed business write (`jobs` table count) and the
committed lease (`bouncer_resources` owner) through the wrapper
surface, not through the originating connection.

### What changed to close [A7]

1. **`packages/bouncer/examples/three_surface_observer.rs`** — Minimal
   standalone binary: opens `Bouncer::open(path)` on the provided file,
   inspects the named resource, prints JSON stdout
   `{"exists":true,"owner":"...","token":N}` or
   `{"exists":false}`. Lives in the `bouncer` crate's `examples/`
   directory; no new public API surface.

2. **`packages/bouncer-py/tests/test_bouncer.py`** — New acceptance row
   `test_three_surfaces_observe_same_state` exercises all three shipped
   surfaces on one file in a single named journey:
   - Python binding claims (`bouncer.open` → `claim`)
   - SQL extension verifies (`bouncer_owner`, `bouncer_token`)
   - Rust wrapper verifies (spawns `cargo run --example three_surface_observer` via `subprocess`)
   - SQL extension releases (`bouncer_release`)
   - Python binding observes no owner (`inspect`)
   - Rust wrapper observer sees `{"exists":false}`
   - Python binding reclaims; token strictly increases
   - SQL extension verifies new owner+token
   - Rust wrapper observer sees new owner+token

   This is direct proof, not pairwise composition. Each surface
   asserts the other two in a single file, single test.

### What passed after response changes

- `cargo test -p bouncer --test user_journeys`: 7/7 pass (Journey 6
  now uses fresh observer)
- `cargo test -p bouncer-core -p bouncer` (full Rust suites): 116/116 pass
- `make test-python`: 23/23 pass (20 existing + 2 previously added + 1 new)

### Decisions

- **D4**: The `three_surface_observer` example is built as an example
  binary, not a test-support crate or inline Rust code in the Python
  test, because invoking the actual Rust wrapper surface from Python
  requires a compiled Rust process. An example binary is the
  smallest surface expansion: one file, `cargo run`-addressable,
  internal only, no new public API.
- **D5**: The three-surface journey shells out to `cargo run` three
  times (claim → inspect, release → inspect, reclaim → inspect).
  The first call compiles the example; subsequent calls reuse the
  cached binary. This is acceptable for an acceptance test.
- **D6**: No `SYSTEM.md` or `CHANGELOG.md` updated yet; changes are
  in response to review findings, not new phase scope.

## Implementation Closeout 1

Session:
- A

Closeout actions:

- updated `SYSTEM.md` so the proved baseline now includes the
  acceptance-layer claims that are directly closed through public
  surfaces
- updated `ROADMAP.md` so Phase 015 is reflected in current status and
  no longer appears as pending work
- updated `CHANGELOG.md` with a landed Phase 015 entry
- updated `commits.txt` with the verification evidence currently on
  record

Verification re-run at closeout:

- `cargo test -p bouncer --test user_journeys`
- `cargo test -p bouncer-core -p bouncer`
- `make test-rust`
- `make test-python`

Status:

- Phase 015 is closed locally.
- No implementation commit hash is recorded yet; add it to
  `commits.txt` when the local work is committed.
