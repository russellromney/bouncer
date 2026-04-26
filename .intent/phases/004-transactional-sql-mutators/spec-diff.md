What changes:

- Bouncer's SQL mutators stop being autocommit-only helpers.
- `bouncer_claim`, `bouncer_renew`, and `bouncer_release` must work both:
  - in autocommit mode, and
  - inside an already-open explicit SQL transaction on the caller's
    connection.
- The SQL surface must therefore match the Honker-family transactional
  extension model: a caller should be able to combine a business write
  and a `bouncer_*` mutator in one SQLite transaction and have commit or
  rollback apply to both.
- `bouncer-honker` gets `pub(crate)` transaction-aware internal helpers
  that take `&Connection`, do not open or commit a transaction, and are
  callable from the SQL registration layer on the current connection.
- The existing Rust public helpers keep their current behavior by
  wrapping those transaction-aware helpers in `BEGIN IMMEDIATE` when
  they are called directly.
- The SQL function names, arguments, and return shapes do not change.
- In the in-transaction SQL path, lock-upgrade timing follows the
  caller's outer transaction mode. Callers who want the same up-front
  writer claim as today's direct Rust path must begin their outer
  transaction with `BEGIN IMMEDIATE` themselves.

What does not change:

- Phase 004 does not change Phase 001 lease semantics.
- Phase 004 does not change the Phase 003 SQL function names.
- Phase 004 does not add implicit `now()` SQL helpers.
- Phase 004 does not add new language bindings.
- Phase 004 does not change Bouncer into a scheduler or workflow system.
- Phase 004 does not weaken the autocommit path's current
  `BEGIN IMMEDIATE` behavior for direct Rust callers.

How we will verify it:

- `bouncer_claim`, `bouncer_renew`, and `bouncer_release` still work in
  autocommit mode.
- Those same SQL mutators succeed inside an already-open explicit SQL
  transaction instead of failing with SQLite's nested-transaction error.
- A rollback of an explicit transaction drops both the caller's write
  and the lease mutation.
- A commit of an explicit transaction preserves both the caller's write
  and the lease mutation.
- Multiple Bouncer SQL mutators in one explicit transaction commit or
  roll back together.
- Read helpers (`bouncer_owner`, `bouncer_token`, and any SQL read path
  used to inspect liveness) still work inside an explicit transaction.
- A known lease-state scenario (claim -> busy -> expired -> takeover ->
  release -> reclaim) still produces the same outcomes when invoked from
  within an explicit transaction.
- The Rust wrapper behavior remains unchanged.
- SQL/Rust interop still works on the same database file after the
  refactor.
