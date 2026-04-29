# Bouncer Roadmap

## Summary

Bouncer should be the sharpest "who owns this right now?" primitive for the Honker family.

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
- a first Rust wrapper crate now exists in `packages/bouncer`
- the wrapper stays thin, keeps bootstrap explicit, and leaves time-ordering concerns out of scope
- a first SQLite loadable-extension crate now exists in `bouncer-extension`
- the first `bouncer_*` SQL surface reuses `bouncer-honker` semantics and keeps `now_ms` explicit
- SQL/Rust interop is now proven on one database file
- SQL mutators now participate in caller-owned explicit transactions and savepoints instead of failing with SQLite's nested-transaction error
- deferred multi-connection writer contention is now pinned as a lock/busy failure in the in-transaction SQL path
- borrowed Rust mutators now follow the same transaction model as the SQL extension instead of tripping nested-transaction errors on caller-owned transactions
- the core now exposes explicit public `*_in_tx` helpers with a fail-fast transaction-state guard
- the wrapper now exposes a sanctioned `Bouncer::transaction()` handle
  with checked `BEGIN IMMEDIATE` semantics and same-wrapper exclusivity

The intended model is:

- `honker`
  generic queue / wake / retry substrate
- `bouncer-honker`
  Bouncer-specific schema and SQLite contract
- `bouncer-extension`
  shared SQLite-facing SQL boundary
- `bouncer`
  thin language bindings

## Next build steps

1. Do one focused hardening pass on the settled core surfaces:
   savepoint surface, cross-connection durability proof, fragile timing
   tests, file-size cleanup, and clearer default-surface docs.
2. Prove the shape with one non-Rust binding, preferably Python, before
   moving on to broader docs/examples or more bindings.

## Future proposals

### DST-forward (deterministic simulation testing)

Honker and its siblings (bouncer-honker, future queue/retry/scheduler
primitives) should be testable under deterministic simulation: every
source of non-determinism flows through a seam the test harness
controls, and the entire system is replayable from a seed. Inspired
by TigerBeetle's VOPR, FoundationDB's simulator, sled's deterministic
test harness, and Antithesis.

The bar:

- All time is injected. Already true at the `bouncer-honker` core
  (`now_ms` parameter on every function). The wrapper layer must
  preserve this — production callers see a `Clock` default, tests see
  whatever the harness wants.
- All randomness flows through a seeded source. Added per-sibling as
  needed (none today).
- All SQLite I/O is interceptable. Needs a VFS shim or rusqlite hook
  so the harness can inject `SQLITE_BUSY`, `SQLITE_FULL`, partial
  writes, fsync drops, and torn pages.
- All operation scheduling is controllable. A generator that produces
  a sequence of `(operation, conn, args)` with seeded selection, run
  by a single-threaded scheduler that can permute order across
  simulated processes.
- Properties replace scenarios. Invariants like "fencing token never
  decreases," "no two live owners simultaneously," "released →
  reclaimable," "expired → takeover succeeds + token++" run across
  millions of seeds.
- Bug minimization. When a property fails, the seed reproduces, and a
  shrinker reduces to the smallest failing trace.

What lives where:

- honker hosts the simulation harness (clock seam, op generator,
  scheduler, VFS shim, property runner) so siblings inherit it.
- Each sibling (bouncer-honker, future queue/retry/scheduler) provides
  its own operation generator and invariant set.
- Production code stays unchanged. DST is a test-time superpower, not
  a runtime cost.

Implications for current decisions:

- The core already has the important seam (`now_ms` on every function).
- A future wrapper phase may add an injectable clock seam if the
  deterministic-simulation investment becomes concrete. Phase 002 can
  stay thinner as long as it does not hide or replace the core's
  explicit-time contract.
- Reading time from inside SQLite (e.g. `unixepoch()`) defeats the
  injection seam and is therefore inconsistent with this direction.
  Keep all time on the Rust side, behind a `Clock` trait or
  equivalent.

Out of scope for this proposal:

- Multi-machine simulation. Cinch is single-machine; fencing token +
  lease semantics are the cross-machine story.
- Replacing real concurrency tests entirely. DST complements stress
  tests, doesn't replace them.
- OS/network-level fault injection. Lives elsewhere if ever needed.

This is a meaningful infrastructure investment and should land as its
own phase per sibling, with honker landing the harness first.

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
