What changes:

- Bouncer adds a focused hardening phase over the now-settled core
  surfaces before any Python binding work begins.
- The wrapper transaction surface gains an explicit savepoint story.
- The wrapper transaction surface gains at least one cross-connection
  durability proof after commit.
- Fragile sleep-based transaction tests are reduced or replaced where
  practical.
- The wrapper docs and system model get clearer about the recommended
  default surfaces:
  - `Bouncer` for simple autocommit calls
  - `Bouncer::transaction()` for sanctioned atomic business-write +
    lease-mutation work
  - `BouncerRef` for caller-owned connection scenarios
- `packages/bouncer/src/lib.rs` should be split so the repo stays under
  the file-size guideline.

What does not change:

- Phase 007 does not add a Python binding yet.
- Phase 007 does not change lease semantics.
- Phase 007 does not redesign the SQL extension surface.
- Phase 007 does not commit to making Bouncer match Honker's exact
  syntax if the current handle-based surface remains cleaner.

How we will verify it:

- savepoint behavior is exposed and tested on the wrapper transaction
  surface, or explicitly resolved in another equally clear sanctioned
  API shape
- at least one transaction-handle commit is observed from a fresh
  connection to the same database file
- the file split preserves test coverage and semantics
- docs and baseline system text clearly state the recommended default
  surface for common caller shapes
