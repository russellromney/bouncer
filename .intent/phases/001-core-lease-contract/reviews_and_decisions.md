# Reviews And Decisions

This file is append-only.

- Review rounds are written in Session B.
- Decision rounds are written in Session A in response to findings.
- Earlier critique should stay visible even after follow-up fixes land.

## Review Round 001

Target:
- plan review

Session:
- B

Model family:
- not recorded in this phase history

Artifacts reviewed:
- `spec-diff.md`
- `plan.md`

Verification reviewed:
- planned test matrix only

### Positive conformance review

- [P1] The scope matches the spec diff: one SQLite-backed lease
  contract, one owner, expiry, fencing, and the four core transitions.
- [P2] The plan keeps the work at the Rust contract first, which is the
  right call for a new state machine.
- [P3] The plan is appropriately narrow for the Honker family thesis.
  This is clearly a single-machine ownership primitive, not a
  distributed coordination system in disguise.
- [P4] The verification list is strong enough to make the first release
  meaningful. If the tests land as written, Phase 001 will already
  prove the important lease transitions.

### Negative conformance review

- [N1] `inspect(conn, name) -> Option<LeaseInfo>` was underspecified.
  Without `now_ms`, it cannot reliably answer "who owns this right
  now?" because an expired stored row might still exist in the database.
- [N2] The draft leaned toward deleting rows on release while also
  requiring monotonic fencing tokens. Those two ideas conflict. If the
  row disappears, the next claim has nowhere to continue the token from.
- [N3] `release` did not accept an injected time even though the schema
  records `updated_at_ms` and the rest of the contract is intentionally
  time-explicit for deterministic testing.
- [N4] The spec diff mentioned a thin binding after the core contract
  exists, but the plan is healthier if Phase 001 ends at Rust plus
  tests. Binding work would create noise before the lease model
  settles.

### Adversarial review

- [A1] This becomes dumb if it is just a named lock with a TTL and a new
  noun.
- [A2] If fencing semantics are not preserved across expiry, release,
  and re-claim, Bouncer becomes a cosmetic wrapper around "best effort
  ownership" instead of a real coordination primitive.
- [A3] If `inspect` drifts into a raw row-reader rather than "current
  owner right now," the public story gets muddy fast and users will make
  inconsistent expiry decisions in their own wrappers.
- [A4] If the first phase tries to expose SQL helpers or language
  bindings too early, the repo will calcify around convenience APIs
  before the actual state machine is stable.
- [A5] If release/re-claim semantics are not tested explicitly, the
  product will feel fine in demos and then fail in the first real
  scheduler or leader-election use case.

### Review verdict

- Accepted with follow-up decisions required before coding.

## Decision Round 001

Responding to:
- Review Round 001

Session:
- A

### Inputs

- [P1]
- [P2]
- [P3]
- [P4]
- [N1]
- [N2]
- [N3]
- [N4]
- [A1]
- [A2]
- [A3]
- [A4]
- [A5]

### Decisions

- [D1] Accept [N1]
  Action: define `inspect` as a live-lease read and make it time-aware.
  Targets: `plan.md`

- [D2] Accept [N2]
  Action: preserve resource rows after first claim so fencing state
  survives release and re-claim.
  Targets: `plan.md`

- [D3] Accept [N3]
  Action: make `release` take injected time like the rest of the core
  contract.
  Targets: `plan.md`

- [D4] Accept [N4]
  Action: stop Phase 001 at the Rust contract and tests, with bindings
  deferred.
  Targets: `plan.md`, implementation scope

- [D5] Accept [A1]
  Action: keep the scope at lease state, expiry, fencing, and tests
  rather than growing a generic lock wrapper with a shinier noun.
  Targets: implementation scope

- [D6] Accept [A2], [A3], and [A5]
  Action: treat fencing continuity, live-lease inspection, and explicit
  release/re-claim semantics as part of the Phase 001 contract, not as
  optional convenience behavior.
  Targets: `plan.md`, tests

- [D7] Accept [A4]
  Action: defer SQL helpers and bindings until the state machine feels
  settled.
  Targets: docs language, future phases

### Verification

- `plan.md` now contains the accepted clarifications:
  time-aware `inspect`, row-preserving release semantics, injected time
  on `release`, and Rust-core-only Phase 001 scope

### Decision verdict

- Ready for implementation.

## Review Round 002

Target:
- implementation review

Session:
- B

Model family:
- not recorded in this phase history

Artifacts reviewed:
- `spec-diff.md`
- `plan.md`
- `bouncer-honker/src/lib.rs`
- tests

Verification reviewed:
- `cargo test`

### Positive conformance review

- [P5] The implementation matches the reviewed plan closely.
- [P6] `bouncer-honker` now owns one durable row shape for lease state,
  one schema bootstrap path, one Rust API for lease transitions, and one
  Rust test suite.
- [P7] The implemented schema matches the clarified plan: resource rows
  persist after first claim, `owner` and `lease_expires_at_ms` are
  nullable for released state, and fencing continuity is preserved
  across re-claim.
- [P8] `inspect` is time-aware, which keeps the public meaning aligned
  with the product question: "who owns this right now?"
- [P9] `claim`, `renew`, and `release` run inside `BEGIN IMMEDIATE`
  transactions, which is the right first step for an ownership
  primitive that must serialize writers.
- [P10] The tests cover the main happy-path and rejection-path semantics
  from the plan: absent, expired, released, first claim, busy claim,
  takeover, owner renew, non-owner renew, owner release, non-owner
  release, and TTL validation.

### Negative conformance review

- [N5] The plan explicitly called out rejecting renewal of an
  already-expired lease in Phase 001. The code appears to do that by
  treating expired rows as having no live lease, but the test suite does
  not pin that exact behavior yet.
- [N6] The current tests are all single-connection in-memory tests.
  That proves the state machine logic, but it does not yet prove the
  same semantics across multiple SQLite connections pointed at the same
  database file, which is the practical environment Bouncer is meant
  for.

### Adversarial review

- [A6] The current implementation is not dumb, because it solves a real
  single-machine coordination problem without inventing distributed
  systems theater.
- [A7] The repo would become dumb if this Phase 001 core were oversold
  as "leader election" or a complete fencing solution. Right now it is
  a durable lease state machine with a monotonic token, not a full
  end-to-end safety story.
- [A8] The fencing token is useful only if downstream consumers actually
  compare or persist it. Phase 001 creates the token correctly, but it
  does not yet teach callers how to use the token to reject stale
  actors.
- [A9] The biggest remaining confidence gap is not the SQL itself; it is
  proving the contract under realistic multi-connection access. Until
  that is tested, the implementation is convincing, but not
  battle-proven.

### Review verdict

- Accepted with follow-up test hardening.

## Decision Round 002

Responding to:
- Review Round 002

Session:
- A

### Inputs

- [P5]
- [P6]
- [P7]
- [P8]
- [P9]
- [P10]
- [N5]
- [N6]
- [A6]
- [A7]
- [A8]
- [A9]

### Decisions

- [D8] Accept [N5]
  Action: add an explicit expired-renew test.
  Targets: tests

- [D9] Accept [N6]
  Action: add file-backed multi-connection tests for lease visibility,
  contention, and expired handoff.
  Targets: tests

- [D10] Accept [A6] and [A7]
  Action: keep Phase 001 framed as a durable single-machine lease state
  machine rather than overselling it as a complete leader-election
  story.
  Targets: docs language

- [D11] Defer [A8]
  Action: keep fencing-token usage guidance for a later phase or future
  binding/docs work.
  Targets: future phases

- [D12] Accept [A9]
  Action: close the main confidence gap by adding realistic
  multi-connection tests instead of expanding mutation logic.
  Targets: tests

### Verification

- Added a dedicated expired-renew test.
- Added file-backed multi-connection tests that prove lease visibility,
  contention, and expired handoff across separate SQLite connections to
  the same database file.
- `cargo test` passes after the follow-up tests land.
- 16 unit tests now pass in `bouncer-honker`.

### Decision verdict

- Phase 001 accepted at the Rust-core level.

## Notes

- This file was normalized after the phase from an older review format
  into the current `reviews_and_decisions.md` shape.
- The substance of the earlier critique was preserved while findings and
  responses were assigned stable IDs.
- Future phases should record the actual Session B model family when it
  is known.
