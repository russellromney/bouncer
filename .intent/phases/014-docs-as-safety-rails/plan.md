# Plan

Phase: 014 — docs as safety rails

Session:
- A

## Goal

Convert the proved but subtle behavior from Phases 012 and 013 into
short, durable guidance that helps callers choose the right surface and
avoid the most likely misuse patterns, while adding a direct
pragma-neutrality matrix so the docs claim is actually backed by tests.

## Context

The code and tests now prove several sharp edges that are easy to get
wrong from memory:

- lease busy is not the same thing as SQLite lock contention
- autocommit and caller-owned transactions have different lock-posture
  timing
- `BEGIN IMMEDIATE` is a policy choice callers may need to make
- Bouncer does not own pragma policy like `busy_timeout` or
  `journal_mode`
- fencing safety beyond SQLite depends on downstream token checks
- strict bootstrap validation now rejects drifted persisted schema

Those are all correct today, but a fast reader can still miss them or
over-infer policy that Bouncer intentionally does not own. The docs can
say "Bouncer is pragma-neutral," but today that is only indirectly
supported by code inspection plus Phase 012 behavior rows. This phase
should make that claim direct.

## References

- `README.md`
- `SYSTEM.md`
- `ROADMAP.md`
- `CHANGELOG.md`
- `.intent/phases/012-sqlite-behavior-matrix/*`
- `.intent/phases/013-schema-and-data-integrity-hardening/*`

## Proposed implementation approach

1. Audit the user-facing docs for places where Phase 012/013 behavior
   is implied but not stated plainly.
2. Add one compact "choosing a surface" explanation that keeps the
   SQL-extension path primary for callers who already own a connection.
3. Add a dedicated pragma-neutrality matrix with file-backed rows that
   pre-set concrete pragma values, run a Bouncer operation, and
   re-read the pragma state afterward.
   Pin the matrix contract to:
   - file/persistent:
     - `journal_mode`
     - `synchronous`
   - connection-local:
     - `busy_timeout`
     - `locking_mode`
     - `foreign_keys`
   Pin the surface coverage to these rows:
   - `bouncer-core/tests/pragma_matrix.rs`
     - core `bootstrap_bouncer_schema`
     - one core autocommit mutator
     - one core `*_in_tx` mutator inside caller-owned transaction
     - SQL registration plus `bouncer_bootstrap()`
     - one SQL mutator in autocommit mode
     - one SQL mutator inside caller-owned transaction/savepoint
   - `packages/bouncer/tests/pragma_matrix.rs`
     - wrapper `Bouncer::bootstrap()`
     - one borrowed-path mutator
     - one `Bouncer::transaction()` row
     - one typed savepoint row
   Verification shape:
   - every row uses a fresh tempdir and file-backed DB
   - re-read connection-local pragmas on the same connection after the
     operation
   - re-read `journal_mode` and `synchronous` on the same connection
     after the operation
   - re-read `journal_mode` and `synchronous` on a fresh connection
     against the same file to prove no hidden rewrite landed
4. Add one compact troubleshooting/safety section covering:
   - lease busy vs SQLite busy/locked
   - `BEGIN IMMEDIATE` guidance
   - pragma ownership (`busy_timeout`, `journal_mode`)
   - fencing-token obligations
   - strict bootstrap drift rejection
5. Keep `SYSTEM.md` as the proved baseline and use `README.md` and any
   package README updates for more operator-facing wording.
6. Keep production changes narrowly scoped to docs plus test-only
   matrix support. Do not widen lease semantics or connection policy.

## Acceptance

- The main docs tell a consistent story about:
  - lease busy versus SQLite lock-class failure
  - when Bouncer owns the transaction versus when the caller does
  - who owns pragma policy
  - who must enforce fencing tokens downstream
  - which public surface to choose
- The repo now has a direct pragma-neutrality matrix proving that the
  pinned caller-owned `journal_mode`, `synchronous`,
  `busy_timeout`, `locking_mode`, and `foreign_keys` settings survive
  bootstrap and lease operations unchanged across the sanctioned core,
  SQL, and wrapper surfaces.
- The docs do not imply a migration engine, retry policy, or pragma
  defaults that Bouncer does not actually own.
- Any new examples or claims remain within the current proved baseline.

## Tests and evidence

- `cargo test -p bouncer-core`
- `cargo test -p bouncer`
- `cargo test -p bouncer-core --test pragma_matrix`
- `cargo test -p bouncer --test pragma_matrix`
- `make test-rust`
- `make test` if production code changes

## Traps

- Do not let docs silently widen the product scope.
- Do not invent operational policy where Bouncer is intentionally
  pragma-neutral.
- Do not let the pragma-neutrality claim drift into "all SQLite pragmas
  forever"; keep the contract pinned to the explicit five-pragmas set
  unless a later phase expands it deliberately.
- Do not bury the fencing-token obligation under convenience language.
- Do not duplicate long explanations across files if one short
  cross-link keeps them aligned better.

## Files likely to change

- `README.md`
- `SYSTEM.md`
- `bouncer-core/tests/pragma_matrix.rs`
- `packages/bouncer/tests/pragma_matrix.rs`
- `packages/bouncer/README.md`
- `packages/bouncer-py/README.md`
- `.intent/phases/014-docs-as-safety-rails/*`

## Areas that should not be touched

- core lease semantics
- new wrapper/API work
- migration machinery
- new bindings

## Commands

- `cargo test -p bouncer-core`
- `cargo test -p bouncer`
- `make test-rust`
- `make test`
