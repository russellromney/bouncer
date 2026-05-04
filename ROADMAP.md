# Litelease Roadmap

## Summary

Litelease should be the sharpest "who owns this right now?" primitive for
SQLite apps.

Its job is not to become a scheduler or workflow system. Its job is to
provide a durable lease with expiry and fencing in the same SQLite file
the app already uses.

## Intent artifacts

- `SYSTEM.md` is the current English model of Litelease.
- `CHANGELOG.md` records what has landed.
- future meaningful changes should add phase records with spec diffs,
  plans, and `reviews_and_decisions.md`.

## Current status

Litelease now has a real shipped baseline: core lease semantics, a SQLite
extension surface, a Rust wrapper, and a layered proof
stack that covers semantics, SQLite behavior, integrity hardening,
pragma-neutrality, user-shaped acceptance, release-shaped extension
smoke proof, and repeated public-surface stress.

`SYSTEM.md` is the current English model of that baseline.
`CHANGELOG.md` is the detailed record of what has landed so far.

The intended product model is:

- `bouncer-core`
  Litelease schema and SQLite contract
- `bouncer-extension`
  shared SQLite-facing SQL boundary
- `packages/bouncer`
  Rust convenience wrapper

## Next build steps

The next Litelease work should stay small and user-driven, not footprint
driven.

1. **Consumer-driven polish.**
   Only add surface or ergonomics work when a real app pushes on a real
   rough edge.
2. **Distribution follow-through.**
   If people actually adopt the extension path, make the release and
   install story nicer without widening the product surface.
3. **Targeted maintenance proof.**
   Add more proof only when a concrete scary area or real bug justifies
   it.

## Future proposals

### Nested wrapper savepoints

The Rust wrapper now has one sanctioned savepoint level through
`Transaction::savepoint()`. Nested savepoints are a plausible future
ergonomic surface, especially if a binding wants nested context
managers or an ORM/framework integration needs local rollback scopes
inside a larger transaction.

Do not add this just because SQLite supports it. Add it when a caller
story needs it, and keep the same terminal handle shape:

- opening a nested boundary borrows the parent savepoint mutably
- `commit(self)` releases the nested savepoint
- `rollback(self)` rolls back to and releases the nested savepoint
- outer rollback still discards all nested work

### Surface posture

Litelease is not trying to grow a large binding matrix.

The intended surface story is:

- the SQL extension is the base interoperability surface
- the Rust wrapper is the primary convenience layer for Rust
- Python should use `sqlite3` plus the SQL extension examples rather
  than a separate package surface
- new bindings should only exist when a real consumer needs one and the
  SQL extension alone is meaningfully awkward

That means future binding work should clear a high bar:

- a real user or product need
- a materially better caller experience than raw SQL
- no duplication of an already-good caller-owned connection story

If the SQL extension is already a clean fit for a language, that is
usually enough.

### Stress and fault proof

Litelease already has good direct proof for semantics, SQLite behavior,
pragma-neutrality, integrity hardening, and user-shaped journeys.

Future proof work should stay proportional:

- add more targeted multi-connection and boundary stress where it buys
  confidence
- consider narrower SQLite fault injection only if specific failure
  modes are worth pinning
- avoid turning the roadmap into a commitment to a heavyweight
  simulator program unless the lightweight proof stack stops being
  enough

## V1 nouns

- resource
- owner
- lease
- fencing token

## Success criteria

- one current owner per named resource
- expiry is durable and inspectable
- fencing token increments on successful claim
- bindings do not reimplement semantics
