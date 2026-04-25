# Bouncer Roadmap

## Summary

Bouncer should be the sharpest "who owns this right now?" wrapper on top of Honker.

Its job is not to become a scheduler or workflow system. Its job is to provide a durable lease with expiry and fencing in the same SQLite file the app already uses.

## Intent artifacts

- `SYSTEM.md` is the current English model of Bouncer.
- `CHANGELOG.md` records what has landed.
- future meaningful changes should add `.intent/` records with spec diffs, plans, and `reviews_and_decisions.md`.

## Current status

The repo now has a real Phase 001 Rust core:

- `bouncer-honker` owns the first SQLite schema
- the core contract exposes `inspect`, `claim`, `renew`, and `release`
- fencing tokens are monotonic across expiry, release, and re-claim
- Rust tests pin the current semantics
- bindings do not exist yet

The intended model is:

- `honker`
  generic queue / wake / retry substrate
- `bouncer-honker`
  Bouncer-specific schema and SQLite contract
- `bouncer`
  thin language bindings

## Next build steps

1. Add one tiny binding that delegates directly to the Rust core contract.
2. Decide whether the first public binding should expose explicit `now_ms` or hide it behind friendlier helpers.
3. Re-evaluate whether a SQLite loadable-extension surface should arrive after the binding, not before.
4. Explore how Honker scheduler ownership could eventually depend on Bouncer without introducing circular product boundaries.

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
