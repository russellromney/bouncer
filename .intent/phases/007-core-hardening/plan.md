# Plan

## Goal

Harden the settled core surfaces before starting the Python binding.

By the end of Phase 006, Bouncer should have:

- core lease semantics pinned
- honest borrowed transaction participation
- honest SQL transaction participation
- a sanctioned wrapper-owned transaction handle

What remains are the gaps and rough edges that are too real to ignore
but too broad to keep smuggling into Phase 006.

## Phase outcome

At the end of Phase 007, Bouncer should have:

- a clear sanctioned savepoint story on the wrapper transaction surface
- at least one cross-connection durability proof for the wrapper
  transaction handle
- fewer fragile sleep-based timing tests where deterministic alternatives
  exist
- wrapper/system docs that say plainly which public surface is the
  recommended default for which use case
- `packages/bouncer/src/lib.rs` split into smaller files without
  changing behavior

At the end of Phase 007, Bouncer should not have:

- a Python binding yet
- a full deterministic simulation seam yet
- a requirement to exactly mirror Honker's transaction syntax

## Mapping from spec diff to implementation

The spec diff says the remaining work is hardening, not new product
surface.

So the implementation plan should produce:

1. the minimum sanctioned savepoint surface or explicit decision that
   closes the gap cleanly
2. at least one fresh-connection durability test for the transaction
   handle
3. one cleanup pass over fragile timing tests
4. a file split of `packages/bouncer/src/lib.rs`
5. clearer docs about default-surface recommendations

## Phase decisions already made

- This phase happens before Python.
- The point is to harden and clarify, not to widen the product in three
  directions at once.
- Family coherence means shared principles, not necessarily identical
  internal shapes.

## Proposed approach

### 1. Finish the transaction story

Decide and implement the savepoint shape for the wrapper-owned
transaction surface.

Likely options:

- `Transaction::savepoint()` plus a `Savepoint` handle
- or another equally explicit sanctioned nested-boundary API

### 2. Strengthen proof

Add a fresh-connection durability test for the transaction handle.

Review the sleep-based semantic tests and replace what can be replaced
without bloating the wrapper surface or violating current explicit-time
choices.

### 3. Make the wrapper easier to maintain

Split `packages/bouncer/src/lib.rs` so the repo stays within the coding
standard file-size guideline.

### 4. Clarify the public story

Update docs and baseline system text so callers can tell:

- when `Bouncer` is enough
- when `Bouncer::transaction()` is the right path
- when `BouncerRef` is the intentionally lower-level surface

## Files likely to change

- `.intent/phases/007-core-hardening/*`
- `packages/bouncer/src/lib.rs`
- `packages/bouncer/src/*.rs`
- `packages/bouncer/README.md`
- `SYSTEM.md`
- `ROADMAP.md`

## Areas that should not be touched

- Python binding code
- `bouncer-honker` lease semantics
- SQL function names

## Risks and assumptions

- The main risk is letting "hardening" turn into a giant umbrella
  refactor. Keep each fix directly tied to an already-known gap.
- The second risk is spending too much effort on family-shape symmetry
  instead of caller clarity.
