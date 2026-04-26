# Plan

## Goal

Decide whether Bouncer should add a SQLite SQL/loadable-extension
surface now that the first Rust wrapper exists.

## Phase outcome

At the end of Phase 003, Bouncer should have:

- a committed answer on whether the SQL surface is the next right move
- a narrower product story around that answer
- either:
  - a follow-on implementation plan for the minimal SQL surface, or
  - an explicit defer/reject decision with the next better phase named

At the end of Phase 003, Bouncer should not have:

- a half-implemented SQL surface
- a widened lease contract
- any hidden pressure for Honker to absorb Bouncer again

## Mapping from spec diff to implementation

The spec diff says Phase 003 is a decision phase, not an implementation
phase.

So the implementation plan should produce exactly three things:

1. a reviewable argument for or against the SQL surface
2. a concrete yes/no decision in `reviews_and_decisions.md`
3. if yes, a small next-phase contract; if no, a clear next build step

## Questions this phase must answer

- Does a SQL surface strengthen the single-machine SQLite story, or does
  it just duplicate the Rust wrapper too early?
- Is the SQL surface mainly for extension users, or mainly for future
  cross-language bindings?
- If SQL exists, what is the smallest honest surface?
- If SQL does not exist yet, what should Bouncer build next instead?

## Candidate SQL surface

If the answer trends yes, the likely minimum surface is:

- `bouncer_claim(name, owner, ttl_ms, now_ms?)`
- `bouncer_renew(name, owner, ttl_ms, now_ms?)`
- `bouncer_release(name, owner, now_ms?)`
- `bouncer_owner(name, now_ms?)`
- `bouncer_token(name)`

This is only a candidate set for evaluation, not a commitment.

## Build order

### 1. Read the current baseline

- use Phase 001 and Phase 002 artifacts
- use `SYSTEM.md`, `README.md`, and `ROADMAP.md`

### 2. Evaluate product shape

- ask whether SQL is actually the next best public surface
- ask whether SQL exposure would clarify or muddy the Bouncer story

### 3. Evaluate technical shape

- if SQL exists, decide whether explicit-time variants are acceptable or
  whether time handling becomes too awkward
- ask what test matrix would be required to keep SQL semantics aligned
  with the core

### 4. Record the decision

- write a proper plan review
- write a decision round that either opens a SQL implementation phase or
  defers SQL clearly

## Files likely to change

- `.intent/phases/003-sql-surface-decision/*`
- `ROADMAP.md`

## Areas that should not be touched

- `bouncer-honker` lease semantics
- `packages/bouncer` wrapper behavior
- Honker integration

## Risks and assumptions

- SQL may sound more "SQLite-native" while still being the wrong next
  surface
- time handling often gets uglier in SQL than in Rust, especially if the
  project still wants deterministic simulation later
- a second public surface is only worth it if it truly broadens the
  product, not if it just repeats the Rust wrapper in a clumsier form
