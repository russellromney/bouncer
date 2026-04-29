# Reviews And Decisions

This file is append-only.

- Review rounds are written in Session B.
- Decision rounds are written in Session A in response to findings.
- Do not rewrite earlier review text to make it look resolved.

## Decision Round 001

### Responding to

- human observation that `bouncer-honker` implies Bouncer depends on
  Honker
- planned future direction where Honker may instead depend on Bouncer
  for scheduler ownership or other coordination

### Decisions

- [D1] Rename the core crate and directory to `bouncer-core`.
  Target:
  - `Cargo.toml`
  - `bouncer-core/Cargo.toml`
  - dependency declarations
  - imports

- [D2] Do not change lease semantics, SQL function names, table names,
  or wrapper API names in this phase.
  Target:
  - code
  - tests

- [D3] Renumber the Python binding phase to Phase 009 so Python does
  not depend on or document the old crate name.
  Target:
  - `.intent/phases/009-python-binding/*`

- [D4] Historical intent artifacts may retain the old name. Current
  docs and active plans should use `bouncer-core`.
  Target:
  - docs
  - active plans

### Verdict

Proceed with the mechanical rename before Python implementation.
