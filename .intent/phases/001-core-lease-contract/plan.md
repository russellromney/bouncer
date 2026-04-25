# Plan

## Goal

Build the smallest useful Bouncer contract in `bouncer-honker` before any real binding surface exists.

## Phase outcome

At the end of Phase 001, Bouncer should have:

- a real SQLite schema and bootstrap path
- a Rust contract for `claim`, `renew`, `release`, and `inspect`
- a monotonic fencing token
- Rust tests that pin the lease semantics

At the end of Phase 001, Bouncer should not have:

- a polished language binding
- SQL scalar functions exposed through a loadable extension
- any distributed coordination story

## Mapping from spec diff to implementation

The spec diff says Phase 001 introduces:

- named resources
- one current owner
- expiry
- fencing token
- claim / renew / release / inspect

So the implementation plan should produce exactly four things:

1. one durable row shape for lease state
2. one bootstrap function that installs that shape
3. one Rust API that owns the lease transitions
4. one test suite that proves the lease transitions match the diff

## Phase decisions

These are the decisions to use for Phase 001 unless implementation forces a better one:

- resource state lives in one table, likely `bouncer_resources`
- resource name is the primary key
- a resource row persists after the first successful claim so the fencing token can stay monotonic across release and re-claim
- absence of a row means "resource has never been seen before"
- `inspect` means "current live lease for this resource right now," so it must be time-aware
- successful claim sets or advances the fencing token
- takeover of an expired lease increments the fencing token
- release clears current ownership but does not reset fencing state
- time is injected explicitly into the Rust contract for deterministic tests
- user-facing wrappers can hide time injection later; Phase 001 should optimize for a crisp core contract first

## Proposed initial schema

Start with one table:

- `name TEXT PRIMARY KEY`
- `owner TEXT`
- `token INTEGER NOT NULL`
- `lease_expires_at_ms INTEGER`
- `created_at_ms INTEGER NOT NULL`
- `updated_at_ms INTEGER NOT NULL`

This is enough for:

- one current owner per resource
- expiry
- fencing
- inspection

Notes:

- `owner` and `lease_expires_at_ms` are nullable so a released resource can keep its last fencing token without pretending it is still owned.
- Phase 001 does not need a separate history table, waiting queue, fairness metadata, or scheduler-specific fields.

## Proposed Rust contract

Phase 001 should start with Rust helpers, not bindings.

Suggested core type:

```rust
pub struct LeaseInfo {
    pub name: String,
    pub owner: String,
    pub token: i64,
    pub lease_expires_at_ms: i64,
}
```

Suggested core functions:

- `bootstrap_bouncer_schema(conn)`
- `inspect(conn, name, now_ms) -> Option<LeaseInfo>`
- `claim(conn, name, owner, now_ms, ttl_ms) -> ClaimResult`
- `renew(conn, name, owner, now_ms, ttl_ms) -> RenewResult`
- `release(conn, name, owner, now_ms) -> ReleaseResult`

Suggested result shape:

- `claim` should tell us whether the caller acquired the lease and what the current lease info is
- `renew` should succeed only for the current owner while the lease is still valid
- `release` should succeed only for the current owner
- `inspect` should return `None` for resources with no live lease, including expired or explicitly released rows

The exact enum names can change during implementation. The important thing is that the contract exposes both transition success and current durable lease state.

## Build order

### 1. Shared types and error model

- define `LeaseInfo`
- define Bouncer error type
- define result enums or equivalent return shapes

### 2. Schema bootstrap

- implement the first real `bootstrap_bouncer_schema`
- make it idempotent
- add a schema-version row only if it actually helps Phase 001

### 3. Inspect first

- implement `inspect` before mutation operations
- pin the row-to-`LeaseInfo` mapping
- define and test that expired or released rows do not count as a current owner

### 4. Claim

- implement the "no row yet" acquire path
- implement the "row exists but is currently unowned" acquire path
- implement the "expired row takeover" path
- implement the "still held by someone else" path
- ensure fencing token initialization and increment rules are explicit
- reject non-positive TTL values

### 5. Renew

- renew only for the current owner
- reject renew for non-owner
- reject renew of an already-expired lease in Phase 001 unless implementation strongly argues otherwise
- reject non-positive TTL values

### 6. Release

- release only for the current owner
- clear ownership and lease expiry while preserving the resource row and fencing token
- update `updated_at_ms` from the injected clock

### 7. Tests

- bootstrap is idempotent
- inspect returns `None` for absent resource
- inspect returns `None` for expired resource
- inspect returns `None` for explicitly released resource
- first claim acquires lease
- second claim while valid is rejected
- expired claim takeover succeeds
- released resource can be claimed again
- fencing token starts at a defined value and increments monotonically
- fencing token does not reset after release
- renew succeeds for owner
- renew fails for non-owner
- release succeeds for owner
- release fails for non-owner

### 8. Re-evaluate binding scope

Phase 001 should stop at the Rust contract and tests. A first binding can be evaluated in Phase 002 once the lease semantics feel settled.

## Files likely to change

- `bouncer-honker/src/lib.rs`
- `bouncer-honker/Cargo.toml`
- new Rust test files or inline tests
- later, one thin binding package if the contract settles

## Areas that should not be touched

- Honker itself
- queue / stream / scheduler semantics
- any distributed coordination story

## Risks and assumptions

- fencing semantics may force a slightly richer schema than the README currently implies
- inspection needs time-aware semantics or it stops answering the question Bouncer is supposed to answer
- release semantics must preserve fencing continuity across release and re-claim
- time injection is likely the right choice for deterministic tests, but bindings may want a friendlier wrapper later
- if claim semantics get awkward in pure SQL, that is a sign to keep the first public contract at the Rust layer until the model settles
