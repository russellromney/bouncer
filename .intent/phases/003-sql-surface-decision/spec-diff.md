What changes:

- Phase 003 is not a decision phase anymore. The human decision is that
  Bouncer should add a first SQLite loadable-extension surface next.
- Bouncer gets a real SQL-facing crate that exposes the first SQLite
  helpers on top of the existing `bouncer-honker` core.
- The first SQL contract stays explicit about bootstrap and time. It
  does not read time from inside SQLite.
- The target Phase 003 SQL surface is:
  - `bouncer_bootstrap()`
  - `bouncer_claim(name, owner, ttl_ms, now_ms)`
  - `bouncer_renew(name, owner, ttl_ms, now_ms)`
  - `bouncer_release(name, owner, now_ms)`
  - `bouncer_owner(name, now_ms)`
  - `bouncer_token(name)`
- Successful SQL writes reuse the exact same schema and lease semantics
  already proven at the core layer.

What does not change:

- Phase 003 does not change Phase 001 lease semantics.
- Phase 003 does not add a Python, Node, or other non-Rust binding.
- Phase 003 does not expand Bouncer into a scheduler or workflow system.
- Phase 003 does not commit Honker to depending on Bouncer yet.
- Phase 003 does not add implicit `now()` SQL helpers.

How we will verify it:

- The loadable extension builds and loads into SQLite.
- `bouncer_bootstrap()` is explicit and idempotent.
- SQL `claim` / `renew` / `release` reuse the same core semantics rather
  than reimplementing lease logic.
- File-backed interop tests prove SQL and Rust can operate on the same
  database file and see the same ownership / fencing state.
- SQL helpers require explicit `now_ms` where time matters, so Phase 003
  does not smuggle wall-clock ordering into the contract.
