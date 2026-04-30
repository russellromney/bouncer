# Plan

Phase: 013 — schema and data-integrity hardening

Session:
- A

## Goal

Make Bouncer's current single-table schema and persisted-row contract
fail loudly and predictably under invalid or manually drifted database
state, without changing valid-row lease semantics.

## Context

Phase 012 proved SQLite transaction-posture behavior around the lease
state machine. The next obvious risk surface is integrity: what happens
when a real database file is edited manually, partially migrated, or
loaded from an incompatible older shape.

The current core already has some loud-failure pieces:

- schema bootstrap with `CREATE TABLE IF NOT EXISTS`
- row-level SQLite `CHECK`s for `(owner, lease_expires_at_ms)` pairing
- `ResourceRow::validate()` for impossible loaded rows
- `InvalidTtlMs`, `TtlOverflow`, and `TokenOverflow`

What is still missing is one explicit phase that says which bad states
are impossible by constraint, which are detected by Bouncer, and how
schema drift is classified.

## References

- `SYSTEM.md`
- `ROADMAP.md`
- `.intent/phases/013-schema-and-data-integrity-hardening/spec-diff.md`
- `bouncer-core/src/lib.rs`
- `bouncer-core/tests/sqlite_matrix.rs`
- `bouncer-core/tests/invariants.rs`

## Mapping from spec diff to implementation

- "schema drift fails loudly" maps to a bootstrap-time schema validation
  path folded into `bootstrap_bouncer_schema` for existing
  `bouncer_resources` tables.
- "invalid manual rows fail loudly" maps to targeted file-backed tests
  that insert or mutate rows outside Bouncer, then call the public core
  API and assert structured failure.
- "overflow/TTL edges stay loud" maps mostly to direct core tests with
  explicit `now_ms` and pre-seeded rows near token limits.
- "unusual names/owners remain opaque text" maps to round-trip tests
  across claim/inspect/release using a pinned representative string set.

## Phase decisions

- Keep the phase centered in `bouncer-core`. Add wrapper or SQL-surface
  rows only where the surface exposes a caller-visible failure different
  from core.
- At closeout, flag strict bootstrap validation and any
  `BOUNCER_SCHEMA_VERSION` removal as deliberate behavior changes in
  `CHANGELOG.md`, not as invisible cleanup.
- Treat old schema versions as schema drift until Bouncer has explicit
  migration machinery. Do not invent version upgrades in this phase.
- Fold schema checking into `bootstrap_bouncer_schema`; do not add a
  parallel `verify_bouncer_schema` API in this phase.
- Schema integrity is strict in this phase, not structural-superset:
  existing `bouncer_resources` tables must preserve the exact six-column
  shape Bouncer depends on, the primary-key position on `name`, required
  nullability, and the two load-bearing invariants.
- Declared type matching is exact text in this phase, not affinity-
  tolerant. A column declared as `BIGINT NOT NULL` where Bouncer expects
  `INTEGER NOT NULL` is schema drift by design.
- Add a dedicated public `Error::SchemaMismatch { reason: String }`
  variant instead of overloading `InvalidLeaseRow` for table-shape
  mismatches.
- `SchemaMismatch.reason` is a human-readable diagnostic, not a stable
  contract string. Callers may match on the variant, not on exact
  reason text.
- Keep unusual names/owners broadly allowed. This phase proves opaque
  storage and round-trip behavior for empty string, whitespace,
  newline/tab, Unicode, punctuation/SQL-shaped strings, and a 4 KiB
  UTF-8 string. It does not add trimming, normalization, or ASCII-only
  rules.
- Embedded NUL is explicitly deferred rather than added to the proved
  contract. Current behavior is rusqlite/SQLite's default text-binding
  behavior and is not pinned as Bouncer semantics.
- Treat "huge timestamps" narrowly in this phase: prove overflow and
  near-boundary behavior, but defer any broader timestamp-policy
  decision for large yet internally consistent persisted values.
- Distinguish between:
  - invalid row content
  - incompatible schema/table shape
  - valid-but-rejected lease semantics such as wrong owner or lease busy
- Treat partial application edits as row-level invariant violations
  (owner/expiry mismatch or `token <= 0`), not as a catch-all for schema
  drift.
- Keep `BOUNCER_SCHEMA_VERSION` implicit for now. If it remains
  unreferenced after this work, remove it as dead code rather than
  pretending version storage exists.
- Use file-backed databases for schema-drift and manual-edit cases so
  the proof matches real persisted-state failure modes.
- Keep fixes narrowly scoped to validation, bootstrap checking, and
  targeted tests. Do not redesign the public wrapper surfaces here.
- Bootstrap-vs-bootstrap concurrency on a fresh valid DB is not a new
  proof target in this phase; Phase 012 already owns lock-posture proof.
- Bootstrap inside a caller-owned transaction remains valid in this
  phase; schema validation is read-only and should not pollute caller
  transaction state.
- The wrapper ripple from `core::Error::SchemaMismatch` is expected to
  remain a no-op because `bouncer::Error` already carries
  `core::Error` through `#[from]`, but the wrapper build/test path still
  verifies that assumption.

## Proposed implementation approach

1. Introduce a small schema-validation helper in `bouncer-core` that
   inspects an existing `bouncer_resources` table inside
   `bootstrap_bouncer_schema` before declaring success on preexisting
   state.
   Implementation approach:
   use `PRAGMA table_info(bouncer_resources)` for exact column metadata,
   primary-key position, and nullability, then inspect
   `sqlite_master.sql` for the table's `CREATE TABLE` text and verify
   the two load-bearing CHECK clauses with simple canonical text
   matching. This phase intentionally does not use behavior probes or a
   full SQL parser.
2. Add `Error::SchemaMismatch { reason: String }` for incompatible
   schema drift reporting.
3. Add a new hardening-focused core test file at
   `bouncer-core/tests/integrity.rs` for:
   - incompatible table shape
   - valid-schema bootstrap idempotency
   - valid schema plus existing live lease remaining observable after
     bootstrap
   - missing required columns
   - invalid manual row content
   - token near-overflow
   - TTL edge cases
   - unusual text round-trips from the pinned representative set
4. For invalid persisted-row fixtures that SQLite's current CHECKs would
   reject, create a deliberately broken `bouncer_resources` table shape
   directly in the test database rather than using `writable_schema`.
   The recommended recipe is: create the table manually without the
   relevant CHECK, insert the bad row, then exercise the public API
   against that persisted state.
5. Assert behavior through public API reads and mutators wherever
   possible.
6. Remove `BOUNCER_SCHEMA_VERSION` if the phase still leaves it unused.
7. Add a small wrapper or SQL surface row only if production code
   changes alter what those callers see.

## Acceptance

- Incompatible preexisting schema fails loudly and predictably.
- Bootstrap remains idempotent on an already-valid current-shape schema.
- Bootstrap on a valid current-shape schema with an existing live lease
  leaves that lease observable.
- Invalid persisted rows fail loudly on public core operations.
- Overflow and TTL edge cases are covered directly and remain
  non-mutating on failure.
- Unusual names/owners round-trip as opaque text without hidden
  normalization.
- Valid-row lease semantics remain unchanged.
- `make test-rust` passes.
- `make test` passes if production code changes.
- Python tests still pass if production code changes.

## Tests and evidence

- `cargo test -p bouncer-core`
- `cargo test -p bouncer`
- `make test-rust`
- `make test`

## Traps

- Do not silently repair invalid rows.
- Do not let schema validation become an accidental migration engine.
- Do not tighten name/owner policy casually; broad text acceptance is
  the current intended default unless the spec changes.
- Do not bury schema drift inside generic SQLite "no such column" or
  similar incidental errors if a clearer Bouncer error is practical.
- Do not weaken valid-row behavior while hardening invalid-row paths.

## Files likely to change

- `bouncer-core/src/lib.rs`
- `bouncer-core/tests/*`
- `packages/bouncer/src/lib.rs` only if wrapper error propagation needs
  explicit touch-up
- `.intent/phases/013-schema-and-data-integrity-hardening/*`
- `SYSTEM.md` at closeout
- `CHANGELOG.md` at closeout
- `ROADMAP.md` at closeout

## Areas that should not be touched

- Python binding design
- new migration machinery
- new bindings
- queue/scheduler work

## Assumptions and risks

- SQLite type affinity makes "schema mismatch" fuzzier than strict SQL
  type systems. This phase intentionally chooses the stricter exact-shape
  posture anyway; that is a user-visible compatibility choice, not an
  implementation accident.
- Matching CHECK constraints through `sqlite_master.sql` text is
  intentionally brittle to schema formatting drift. That is acceptable
  in this phase because the schema is tiny and the strict contract is
  the point.
- Some invalid-row cases may already be impossible through SQLite
  constraints and only reachable via manual edits or disabled
  constraint enforcement. That is fine; the point is to prove the loud
  behavior if such a row exists.
- `SchemaMismatch` is a deliberate public API addition in this phase,
  not an accidental side effect.

## Commands

- `cargo test -p bouncer-core`
- `cargo test -p bouncer`
- `make test-rust`
- `make test`
