# Spec Diff

Phase: 013 — schema and data-integrity hardening

Session:
- A

## What changes

- Bouncer gains an explicit hardening phase for invalid persisted rows,
  incompatible existing schema, and edge-value handling.
- This phase broadens proof and detection around failure paths that
  already partly exist today (`InvalidLeaseRow`, `InvalidTtlMs`,
  `TtlOverflow`, `TokenOverflow`). The genuinely new behavior is strict
  schema-drift rejection at bootstrap plus any new detection needed to
  classify incompatible persisted state more clearly.
- The default posture for this phase is loud failure, not silent repair.
  If the on-disk database is incompatible with the proved Bouncer schema
  or contains an invalid lease row, Bouncer returns an error instead of
  guessing how to recover.
- `bootstrap_bouncer_schema(conn)` still creates the current schema when
  Bouncer state is absent. Schema integrity checking is folded into
  `bootstrap_bouncer_schema` itself rather than a separate
  verification-only helper. Calling bootstrap on a database that already
  has a current-shape `bouncer_resources` table remains an idempotent
  no-op success.
- For this phase, the load-bearing schema-integrity contract is strict:
  an existing `bouncer_resources` table must match Bouncer's current
  proved schema exactly enough to preserve the same six columns, column
  order, declared column names, primary-key position on `name`, required
  nullability (`token`, `created_at_ms`, `updated_at_ms` non-null), and
  the two table-level invariants Bouncer depends on:
  - `token >= 1`
  - `(owner IS NULL AND lease_expires_at_ms IS NULL) OR (owner IS NOT
    NULL AND lease_expires_at_ms IS NOT NULL)`
- Extra columns are schema drift in this phase, not a tolerated
  structural superset.
- A new public `Error::SchemaMismatch { reason: String }` variant is an
  intended Phase 013 API addition. Incompatible table shape is not
  folded into `InvalidLeaseRow` or incidental raw SQLite errors.
- For this phase, "old schema version" is treated as one form of schema
  drift. Bouncer does not add migration machinery yet. Any preexisting
  Bouncer-shaped table that is incompatible with the current schema is
  an error until a later phase introduces an upgrade story.
- `BOUNCER_SCHEMA_VERSION` does not gain storage teeth in this phase.
  The proved schema version remains implicit in the exact schema shape,
  and the currently unreferenced constant may be removed as cleanup.
- Lease rows that violate Bouncer's core row invariants remain invalid
  even if they were inserted or edited outside Bouncer. Read and mutator
  surfaces fail loudly on such rows instead of partially operating on
  them.
- Token near-overflow stays a loud error. A claim takeover or reclaim
  that would advance `token` beyond `i64::MAX` fails without mutating
  the row.
- `ttl_ms <= 0` and `now_ms + ttl_ms` overflow remain loud errors and
  gain broader direct coverage in the hardening suite.
- "Huge timestamps" is only in scope in this phase where they interact
  with already-shipped overflow or integrity paths. That means explicit
  coverage for `now_ms + ttl_ms` overflow and token-overflow-adjacent
  persistence. Broader timestamp-policy questions such as rejecting very
  large but internally consistent persisted times, or assigning meaning
  to negative timestamps, are explicitly deferred.
- Unusual resource names and owner strings remain opaque SQLite `TEXT`.
  This phase proves round-trip behavior for a concrete set of practical
  unusual values:
  - empty string
  - leading/trailing whitespace
  - embedded newlines and tabs
  - Unicode beyond basic ASCII, including combining-mark cases
  - punctuation and SQL-shaped strings
  - a long-ish UTF-8 string
- Embedded NUL (`\0`) is explicitly deferred; this phase does not add it
  to the proved contract.
- Partial application edits are treated as invalid persisted state, not
  as a supported recovery path. In this phase that bucket means manual
  row states that violate the core row invariants Bouncer already
  depends on, such as owner/expiry mismatch or `token <= 0`. Schema
  damage such as missing columns is treated as schema drift, not as a
  row-level partial edit.
- The hardening proof lives primarily in `bouncer-core`, with thin
  wrapper and SQL-extension coverage only where the surface exposes a
  caller-visible failure different from core.

## What does not change

- No migration engine.
- No automatic repair or coercion of invalid rows.
- No binding expansion.
- No queue/scheduler behavior.
- No hidden normalization of names or owners.
- No change to the existing lease state machine for valid rows.
- No new timestamp policy beyond the explicitly pinned overflow and
  integrity cases above.
- No dedicated bootstrap-concurrency proof in this phase; bootstrap
  contention remains under SQLite's existing behavior unless a new issue
  is discovered.
- In-scope production changes are limited to the integrity and failure
  surfaces explicitly named in this spec, plus small direct fixes needed
  to support them such as validation branches or the deliberate
  `SchemaMismatch` error addition. Broader lease-semantic changes split
  into a follow-up phase.

## How we will verify it

- Add a dedicated Rust hardening suite for invalid rows, schema drift,
  overflow edges, TTL edges, and unusual text values.
- Cover at least one verified case each for:
  - incompatible preexisting `bouncer_resources` schema failing loudly
  - bootstrap idempotency on an already-valid current-shape schema
  - invalid manual row shape failing loudly on read
  - token near-overflow failing without mutation
  - `ttl_ms <= 0` and expiry overflow remaining loud
  - unusual names/owners round-tripping without normalization
  - partial/manual row edits failing loudly rather than being repaired
- `make test-rust` passes.
- `make test` passes if production code changes.
- Python tests still pass if production code changes.

## Notes

- The goal is to make the current single-table contract harder to use
  incorrectly, not to design a future migration framework.
- This phase should give Phase 014 enough proof to document "what kinds
  of database damage or drift Bouncer rejects, and how loudly."
