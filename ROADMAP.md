# Bouncer Roadmap

## Summary

Bouncer should be the sharpest "who owns this right now?" primitive for
SQLite apps.

Its job is not to become a scheduler or workflow system. Its job is to
provide a durable lease with expiry and fencing in the same SQLite file
the app already uses.

## Intent artifacts

- `SYSTEM.md` is the current English model of Bouncer.
- `CHANGELOG.md` records what has landed.
- future meaningful changes should add phase records with spec diffs,
  plans, and `reviews_and_decisions.md`.

## Current status

Bouncer now has a real shipped baseline: core lease semantics, a SQLite
extension surface, a Rust wrapper, a Python binding, and a layered proof
stack that covers semantics, SQLite behavior, integrity hardening,
pragma-neutrality, and user-shaped acceptance.

`SYSTEM.md` is the current English model of that baseline.
`CHANGELOG.md` is the detailed record of what has landed so far.

The intended product model is:

- `bouncer-core`
  Bouncer-specific schema and SQLite contract
- `bouncer-extension`
  shared SQLite-facing SQL boundary
- `packages/bouncer`
  Rust convenience wrapper
- `packages/bouncer-py`
  Python convenience/demo wrapper

## Next build steps

The next Bouncer work should focus on surface clarity and correctness,
not on expanding footprint.

1. **Python surface and boundary pass.**
   Re-check whether the Python binding is exposing exactly the right
   surface:
   - owned-connection convenience/demo only
   - no shadow caller-owned-connection API
   - docs and examples that point caller-owned Python SQLite users at
     the SQL extension
   - clear parity and non-parity with the Rust wrapper
2. **Cross-surface boundary polish.**
   Keep making the SQL extension feel like the base interoperability
   surface and the Rust/Python wrappers feel like convenience layers
   rather than competing products.
3. **Targeted stress hardening.**
   Add more narrow proof where it buys confidence, especially around
   multi-connection behavior, without committing to a full deterministic
   simulator program unless real bugs justify that investment.

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

Bouncer is not trying to grow a large binding matrix.

The intended surface story is:

- the SQL extension is the base interoperability surface
- the Rust wrapper is the primary convenience layer for Rust
- the Python binding is a convenience/demo layer, not the center of the
  product story
- new bindings should only exist when a real consumer needs one and the
  SQL extension alone is meaningfully awkward

That means future binding work should clear a high bar:

- a real user or product need
- a materially better caller experience than raw SQL
- no duplication of an already-good caller-owned connection story

If the SQL extension is already a clean fit for a language, that is
usually enough.

### Stress and fault proof

Bouncer already has good direct proof for semantics, SQLite behavior,
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
