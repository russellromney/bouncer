# Plan

## Goal

Build the first real Rust binding on top of `bouncer-honker` without
changing the underlying lease contract.

## Phase outcome

At the end of Phase 002, Bouncer should have:

- a new `packages/bouncer` Rust crate in the workspace
- a small ergonomic API that delegates to `bouncer-honker`
- friendlier default time handling for normal callers
- tests that prove the binding and the core interoperate on the same
  SQLite database

At the end of Phase 002, Bouncer should not have:

- a loadable-extension SQL surface
- Python, Node, or other language bindings
- hidden background renewal behavior
- any new lease semantics beyond Phase 001

## Mapping from spec diff to implementation

The spec diff says Phase 002 introduces:

- one thin Rust binding crate
- wrapper-level open/bootstrap behavior
- wrapper-level `inspect`, `claim`, `renew`, and `release`
- system-time defaults on top of the explicit-time core

So the implementation plan should produce exactly five things:

1. one new Rust crate in `packages/bouncer`
2. one small public wrapper type for opening or owning a SQLite
   connection
3. one thin method layer that delegates to `bouncer-honker`
4. one small error/result mapping story that stays close to the core
5. one test suite that proves wrapper/core interoperability

## Phase decisions

These are the decisions to use for Phase 002 unless implementation
forces a better one:

- the first binding is an in-process Rust crate, not a cross-language
  package
- the binding should be ergonomic, not magical
- the binding should use system time by default
- the explicit-time core remains public in `bouncer-honker` for callers
  that need deterministic control
- bootstrap should be explicit and idempotent, not a hidden constructor
  side effect
- the binding should support both an owned wrapper and a borrowed wrapper
  around an existing `rusqlite::Connection`
- the binding should re-export core result shapes rather than invent a
  parallel taxonomy
- Phase 002 should not silently tune `journal_mode`, `busy_timeout`, or
  other SQLite pragmas; connection policy stays with the caller unless a
  later phase introduces that explicitly
- Phase 002 wrapper methods do not need to expose time for ordering.
  Time is for expiry; ordering and stale-actor safety come from SQLite
  writer serialization plus fencing tokens.
- deterministic time control remains available through the explicit-time
  core in `bouncer-honker`; a wrapper-level clock seam is a later
  decision if the simulation direction becomes concrete

## Proposed public shape

Start with one small owned wrapper type plus one borrowed wrapper:

```rust
pub struct Bouncer {
    conn: rusqlite::Connection,
}
```

```rust
pub struct BouncerRef<'a> {
    conn: &'a rusqlite::Connection,
}
```

Suggested surface:

- `Bouncer::open(path) -> Result<Self>`
- `Bouncer::bootstrap(&self) -> Result<()>`
- `Bouncer::as_ref(&self) -> BouncerRef<'_>`
- `BouncerRef::new(conn: &rusqlite::Connection) -> Self`
- `BouncerRef::bootstrap(&self) -> Result<()>`
- `inspect(name) -> Result<Option<LeaseInfo>>`
- `claim(name, owner, ttl) -> Result<ClaimResult>`
- `renew(name, owner, ttl) -> Result<RenewResult>`
- `release(name, owner) -> Result<ReleaseResult>`

The exact type names can change. The important part is that the wrapper
stays thin and obvious.

## Time handling

Phase 001 intentionally required explicit `now_ms` injection.

Phase 002 should make normal usage friendlier while keeping the wrapper
small:

- wrapper methods use system time internally
- core explicit-time functions remain the source of truth underneath
- wrapper tests should stay narrow around the system-time path
- deterministic time-sensitive tests continue at the core layer in
  Phase 002

## Build order

### 1. Workspace wiring

- add `packages/bouncer` as a workspace member
- create its `Cargo.toml`
- depend on `bouncer-honker` and `rusqlite`

### 2. Wrapper shape

- choose the smallest wrapper type that feels honest
- make open/bootstrap behavior explicit
- commit to owned plus borrowed wrapper types
- avoid hiding SQLite too much

### 3. Thin method delegation

- implement wrapper methods for `inspect`, `claim`, `renew`, and
  `release`
- delegate semantics directly to `bouncer-honker`
- keep result mapping boring

### 4. Error model

- re-export core errors and result shapes where practical
- avoid a second deep error taxonomy
- do not add wrapper-only semantic enums unless implementation forces it

### 5. Bootstrap and connection policy

- `open(path)` opens a plain rusqlite connection
- `bootstrap()` is explicit and idempotent
- do not silently set `journal_mode`, `busy_timeout`, or other pragmas
- keep SQLite connection policy with the caller in Phase 002
- if test setup needs connection settings for reliability, put them in a
  test-only helper and document them as harness configuration rather
  than product behavior

### 6. Tests

- open a database through the binding and perform a full lease cycle
- verify `open(path)` does not bootstrap implicitly
- verify wrapper methods fail cleanly before `bootstrap()`
- verify that data written through the binding is visible to the core
- verify that data written through the core is visible to the binding
- verify interop across separate SQLite connections to the same file
- pin fencing-token monotonicity across a wrapper claim and a raw-core
  claim on the same file
- test wrapper bootstrap idempotence
- test TTL-rejection parity with the core
- keep wrapper system-time tests narrow and non-flaky
- keep time-sensitive semantic tests at the core layer in Phase 002

### 7. Docs

- update `packages/bouncer/README.md`
- update the repo README if the new binding surface is ready to mention

## Files likely to change

- `Cargo.toml`
- `packages/bouncer/Cargo.toml`
- `packages/bouncer/src/lib.rs`
- `packages/bouncer/README.md`
- wrapper test files
- Rust tests for the new crate

## Areas that should not be touched

- the Phase 001 lease semantics in `bouncer-honker`
- any distributed coordination story
- SQL helper surfaces
- non-Rust bindings
- hidden connection-policy changes

## Risks and assumptions

- it is easy for a "thin" binding to become a second interpretation
  layer, so the wrapper should stay visually close to the core
- time handling gets flaky quickly if the wrapper depends too much on
  wall clock behavior, which is why the wrapper should not try to prove
  fine-grained ordering via time
- an implicit bootstrap in `open(path)` would be convenient and also
  easy to regret once callers already have their own migration story
- choosing the Rust wrapper shape now will constrain future non-Rust
  bindings, so Phase 002 should make the shape explicit instead of
  punting
- if future deterministic simulation wants a wrapper-level `Clock` seam,
  that should land as its own explicit decision or phase rather than
  sneaking into this one
- if the wrapper shape gets awkward quickly, that is a sign to keep it
  smaller rather than to add more helper magic
