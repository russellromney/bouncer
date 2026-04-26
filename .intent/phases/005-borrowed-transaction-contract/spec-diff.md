What changes:

- `packages/bouncer` stops pretending `BouncerRef` owns the transaction
  boundary.
- `BouncerRef::claim`, `BouncerRef::renew`, and `BouncerRef::release`
  must behave like the SQL extension surface:
  - in autocommit mode, they keep using the direct core helpers that
    open `BEGIN IMMEDIATE`
  - inside an already-open explicit transaction or savepoint, they
    reuse the caller's current transaction state instead of attempting
    a nested transaction
- `bouncer-honker` therefore promotes its transaction-aware lease
  helpers into an explicit public Rust surface with documented
  preconditions and a runtime guard that rejects autocommit misuse.
- This gives direct Rust callers one honest model: whoever owns the
  transaction boundary owns the lock-timing behavior.

What does not change:

- Phase 005 does not change lease semantics.
- Phase 005 does not change the SQL extension surface.
- Phase 005 does not add hidden wall-clock reads.
- Phase 005 does not add a new `bouncer_begin()` helper.
- Phase 005 does not add a full wrapper-owned transaction handle yet.
- Phase 005 does not change `Bouncer::claim`, `Bouncer::renew`, or
  `Bouncer::release`.

How we will verify it:

- `BouncerRef` autocommit behavior remains unchanged.
- `BouncerRef` mutators succeed inside an already-open explicit
  transaction instead of failing with SQLite's nested-transaction
  error.
- A rollback of a caller-owned explicit transaction drops both a
  business write and the `BouncerRef` lease mutation.
- A commit of a caller-owned explicit transaction preserves both.
- Multiple `BouncerRef` mutators in one explicit transaction commit or
  roll back together.
- A compact borrowed-path semantic-stress scenario still produces the
  same token and ownership outcomes as the autocommit path.
- `BouncerRef` also works inside a savepoint without attempting a
  nested transaction.
- The new public core in-transaction helpers are documented as
  caller-owned transaction operations rather than accidental public API.
- Calling a public `*_in_tx` helper on an autocommit connection fails
  fast with an explicit transaction-state error rather than silently
  weakening atomicity.
