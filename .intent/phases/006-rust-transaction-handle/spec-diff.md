What changes:

- `packages/bouncer` adds a sanctioned wrapper-owned transaction path.
- `Bouncer::transaction()` begins `BEGIN IMMEDIATE` and returns a
  Rust transaction handle for atomic business writes plus Bouncer lease
  mutations on one connection.
- `Bouncer::transaction()` mutably borrows the wrapper so normal
  wrapper calls cannot overlap the open transaction on the same
  connection.
- The returned handle exposes the same lease verbs as the wrapper:
  - `inspect`
  - `claim`
  - `renew`
  - `release`
- The returned handle also exposes the underlying SQLite connection so
  callers can do business writes in the same transaction boundary.
- The transaction handle reuses the existing public
  `claim_in_tx` / `renew_in_tx` / `release_in_tx` helpers rather than
  reimplementing lease semantics again.

What does not change:

- Phase 006 does not change lease semantics.
- Phase 006 does not change the SQL extension surface.
- Phase 006 does not remove `BouncerRef`.
- Phase 006 does not add hidden wall-clock reads.
- Phase 006 does not redesign the whole wrapper around closures or a
  `with_transaction(...)` helper yet.

How we will verify it:

- `Bouncer::transaction()` opens a usable Rust transaction handle with
  `BEGIN IMMEDIATE` semantics.
- Opening a transaction now enforces same-wrapper exclusivity at the
  Rust borrow level rather than only through a runtime SQLite error.
- A lease mutation plus a business write commit together through the new
  handle.
- A lease mutation plus a business write roll back together through the
  new handle.
- Multiple lease mutations through the new handle commit or roll back
  together.
- Dropping the new handle without `commit()` rolls back the lease
  mutation.
- The transaction handle preserves the same lease/token semantics as the
  existing autocommit path.
