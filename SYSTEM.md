# Bouncer System

Bouncer is a single-machine lease and ownership primitive on top of Honker.

## Current baseline

- The repo currently contains:
  - `README.md`
  - `ROADMAP.md`
  - `CHANGELOG.md`
  - this `SYSTEM.md`
  - a real `bouncer-honker` crate
- `bouncer-honker` installs a `bouncer_resources` table.
- `bouncer-honker` exposes Rust helpers for `inspect`, `claim`, `renew`, and `release`.
- A resource row persists after its first successful claim so the fencing token can stay monotonic across expiry, release, and re-claim.
- `inspect(name, now_ms)` answers whether there is a live lease right now; expired or released rows do not count as owned.
- `renew` succeeds only for the current live owner.
- `release` succeeds only for the current live owner and clears ownership without resetting fencing state.
- The current proof includes file-backed multi-connection tests against a shared SQLite database file.
- There are no real bindings yet.
- The Phase 001 lease semantics are covered by Rust tests.

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
- This repo does not yet expose a polished language binding or a SQLite loadable-extension surface.
