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
- bootstrap should happen through the binding open path or an explicit
  helper, but not by hidden global side effect
- result types may wrap or re-export the core result shapes as long as
  the semantics stay unchanged

## Proposed public shape

Start with one small wrapper type, probably something like:

```rust
pub struct Bouncer {
    conn: rusqlite::Connection,
}
```

Or, if implementation argues for it, a borrowed/owned split such as:

- `Bouncer`
- `BouncerRef<'a>`

Suggested surface:

- `Bouncer::open(path) -> Result<Self>`
- `Bouncer::from_connection(conn) -> Result<Self>` or a close variant
- `inspect(name) -> Result<Option<LeaseInfo>>`
- `claim(name, owner, ttl) -> Result<ClaimResult>`
- `renew(name, owner, ttl) -> Result<RenewResult>`
- `release(name, owner) -> Result<ReleaseResult>`

The exact type names can change. The important part is that the wrapper
stays thin and obvious.

## Time handling

Phase 001 intentionally required explicit `now_ms` injection.

Phase 002 should make normal usage friendlier:

- wrapper methods use system time internally
- core explicit-time functions remain the source of truth

If implementation wants explicit wrapper variants like `claim_at(...)`
for advanced callers, that is acceptable, but Phase 002 does not require
them.

## Build order

### 1. Workspace wiring

- add `packages/bouncer` as a workspace member
- create its `Cargo.toml`
- depend on `bouncer-honker` and `rusqlite`

### 2. Wrapper shape

- choose the smallest wrapper type that feels honest
- make open/bootstrap behavior explicit
- avoid hiding SQLite too much

### 3. Thin method delegation

- implement wrapper methods for `inspect`, `claim`, `renew`, and
  `release`
- delegate semantics directly to `bouncer-honker`
- keep result mapping boring

### 4. Error model

- decide whether the wrapper re-exports core errors or wraps them
- avoid a second deep error taxonomy

### 5. Tests

- open a database through the binding and perform a full lease cycle
- verify that data written through the binding is visible to the core
- verify that data written through the core is visible to the binding
- pin the wrapper's system-time path without sleeping or flaky timing
  where possible

### 6. Docs

- update `packages/bouncer/README.md`
- update the repo README if the new binding surface is ready to mention

## Files likely to change

- `Cargo.toml`
- `packages/bouncer/Cargo.toml`
- `packages/bouncer/src/lib.rs`
- `packages/bouncer/README.md`
- Rust tests for the new crate

## Areas that should not be touched

- the Phase 001 lease semantics in `bouncer-honker`
- any distributed coordination story
- SQL helper surfaces
- non-Rust bindings

## Risks and assumptions

- it is easy for a "thin" binding to become a second interpretation
  layer, so the wrapper should stay visually close to the core
- time handling can get flaky if tests depend on wall-clock behavior too
  loosely
- if the wrapper shape gets awkward quickly, that is a sign to keep it
  smaller rather than to add more helper magic
