# Bouncer System

Bouncer is a single-machine lease and ownership primitive for the Honker family.

## Current baseline

- The repo currently contains:
  - `README.md`
  - `ROADMAP.md`
  - `CHANGELOG.md`
  - this `SYSTEM.md`
  - a real `bouncer-honker` crate
- a real `packages/bouncer` Rust wrapper crate
- `bouncer-honker` installs a `bouncer_resources` table.
- `bouncer-honker` exposes Rust helpers for `inspect`, `claim`, `renew`, and `release`.
- A resource row persists after its first successful claim so the fencing token can stay monotonic across expiry, release, and re-claim.
- `inspect(name, now_ms)` answers whether there is a live lease right now; expired or released rows do not count as owned.
- `renew` succeeds only for the current live owner.
- `release` succeeds only for the current live owner and clears ownership without resetting fencing state.
- The current proof includes file-backed multi-connection tests against a shared SQLite database file.
- `packages/bouncer` exposes an owned `Bouncer` wrapper and a borrowed `BouncerRef<'a>`.
- The wrapper requires explicit `bootstrap()` and does not silently create schema state in `open(path)`.
- Wrapper convenience methods use system time for lease expiry bookkeeping only.
- `bouncer-honker` now exposes both transaction-owning Rust helpers
  (`claim`, `renew`, `release`) and caller-owned transaction helpers
  (`claim_in_tx`, `renew_in_tx`, `release_in_tx`).
- The public `*_in_tx` helpers fail fast with `Error::NotInTransaction`
  when called on an autocommit connection.
- Stale-actor safety still flows through SQLite writer serialization and fencing tokens, not through wall-clock ordering.
- Bouncer can only provide the fencing token. Downstream callers must include and compare that token at their external side-effect boundary if they want stale-actor protection beyond SQLite itself.
- The wrapper stays pragma-neutral; callers own connection policy such as `journal_mode` and `busy_timeout`.
- Wrapper tests prove negative bootstrap behavior and wrapper/core interoperability on the same database file.
- `BouncerRef` mutators now mirror the SQL extension's transaction
  model: in autocommit mode they open their own `BEGIN IMMEDIATE`
  through the core helpers, and inside an existing transaction or
  savepoint they reuse the caller's current atomic boundary.
- Wrapper tests now also prove borrowed-path commit/rollback,
  multi-mutator transactions, semantic-stress behavior, and savepoint
  participation on the same database file.
- Contention semantics are still primarily proven at the core layer; the wrapper proves thin delegation, interop, and borrowed transaction participation rather than a new concurrency model.
- a real `bouncer-extension` loadable-extension crate exists in the workspace.
- `bouncer-honker` now also owns the first `bouncer_*` SQL function registration surface via `attach_bouncer_functions`.
- The current SQL surface is:
  - `bouncer_bootstrap()`
  - `bouncer_claim(name, owner, ttl_ms, now_ms)`
  - `bouncer_renew(name, owner, ttl_ms, now_ms)`
  - `bouncer_release(name, owner, now_ms)`
  - `bouncer_owner(name, now_ms)`
  - `bouncer_token(name)`
- SQL and Rust interoperate against the same database file and share the same lease semantics and fencing state.
- The SQL surface keeps time explicit. It does not read `now()` from inside SQLite.
- SQL mutators now work both in autocommit mode and inside an already-open explicit transaction or savepoint on the caller's connection.
- In autocommit mode, mutating SQL helpers preserve the direct Rust path's `BEGIN IMMEDIATE` behavior.
- Inside a caller-owned transaction or savepoint, mutating SQL helpers reuse the current transaction state rather than opening a nested transaction.
- In that in-transaction path, lock-upgrade timing follows the caller's outer transaction mode rather than forcing a new `BEGIN IMMEDIATE`.
- Core tests now prove commit/rollback behavior, multi-mutator transactions, read helpers inside a transaction, semantic-stress behavior inside a transaction, a savepoint rollback path, and a lock/busy failure under deferred multi-connection writer contention.

## Current intent

- Bouncer answers "who owns this named resource right now?" for normal SQLite apps.
- Bouncer is for the single-machine SQLite stack, not distributed coordination.
- Bouncer should stay small, inspectable, and boring.

## Boundaries that already matter

- `SYSTEM.md` should describe only the current proved baseline, not the desired finished system.
- Future semantic changes should be proposed through new `.intent/phases/...` artifacts before the code drifts.
- Honker remains the generic async substrate for the family.

## Non-goals

- This repo is not distributed consensus.
- This repo is not a workflow engine.
- This repo does not yet expose non-Rust language bindings.
