# Plan

## Goal

Implement the first SQLite SQL/loadable-extension surface for Bouncer.

## Phase outcome

At the end of Phase 003, Bouncer should have:

- a real SQLite loadable-extension crate in the workspace
- a minimal SQL surface that reuses the proven `bouncer-honker` core
- file-backed proof that SQL and Rust interoperate on the same database
  file
- docs that explain what the SQL surface is for and where it stops

At the end of Phase 003, Bouncer should not have:

- a widened lease contract
- hidden time reads inside SQLite
- a second implementation of lease semantics
- any hidden pressure for Honker to absorb Bouncer again

## Mapping from spec diff to implementation

The spec diff says the product decision is already made: Bouncer should
ship a first SQL surface next.

So the implementation plan should produce three things:

1. a new loadable-extension crate that registers the first `bouncer_*`
   SQL helpers
2. interop tests proving those helpers share semantics and state with
   the existing Rust surfaces
3. doc updates that describe the SQL surface honestly as the next public
   boundary, not a speculative maybe

## Phase decisions already made

- SQL is the next right move.
- The SQL surface is for both direct SQLite callers and future thin
  bindings in other languages.
- Time stays explicit in SQL. There is no implicit `now()` helper in
  Phase 003.
- The SQL surface stays small and claim-centric. It does not try to
  expose every possible inspection helper at once.

## Target SQL surface

The minimum Phase 003 surface is:

- `bouncer_bootstrap()`
- `bouncer_claim(name, owner, ttl_ms, now_ms)`
- `bouncer_renew(name, owner, ttl_ms, now_ms)`
- `bouncer_release(name, owner, now_ms)`
- `bouncer_owner(name, now_ms)`
- `bouncer_token(name)`

Expected return shape:

- `bouncer_bootstrap()` returns success or SQL error.
- `bouncer_claim(...)` returns the fencing token on success, `NULL` when
  the resource is currently held by another live owner, and SQL error on
  invalid arguments.
- `bouncer_renew(...)` returns the current fencing token on success,
  `NULL` when there is no matching live lease, and SQL error on invalid
  arguments.
- `bouncer_release(...)` returns `1` on success and `0` when there is no
  matching live lease.
- `bouncer_owner(...)` returns the current live owner or `NULL`.
- `bouncer_token(...)` returns the current fencing token for the
  resource, or `NULL` if the resource has never been claimed.

## Build order

### 1. Add the extension crate shape

- add a new workspace member for the SQLite loadable extension
- mirror Honker's "core + extension + thin bindings" shape instead of
  inventing a third architecture

### 2. Register the minimal SQL helpers

- implement `bouncer_bootstrap`, `claim`, `renew`, `release`, `owner`,
  and `token`
- delegate every state transition to `bouncer-honker`
- keep `now_ms` explicit in the SQL signatures

### 3. Prove SQL/Rust interop on one database file

- load the extension into SQLite
- prove bootstrap is explicit and idempotent
- prove SQL writes are visible to the Rust core and Rust writes are
  visible to SQL helpers
- prove fencing monotonicity survives mixed SQL/Rust usage

### 4. Update docs and baseline honestly

- update README / ROADMAP / package docs to describe the new SQL surface
- update `SYSTEM.md` only after tests prove the extension belongs in the
  baseline

## Files likely to change

- `Cargo.toml`
- `bouncer-extension/*`
- `.intent/phases/003-sql-surface-decision/*`
- `README.md`
- `ROADMAP.md`
- `SYSTEM.md`

## Areas that should not be touched

- `bouncer-honker` lease semantics
- `packages/bouncer` wrapper behavior except where interop tests need a
  little shared harness code
- Honker integration

## Risks and assumptions

- loadable-extension packaging is more work than the Rust wrapper, so
  the surface must stay small
- scalar SQL functions have awkward return-shape limits; the Phase 003
  contract should prefer boring scalar results over clever JSON
- explicit `now_ms` will look a little uglier in SQL, but that is still
  better than hiding time inside SQLite and breaking the deterministic
  simulation direction
