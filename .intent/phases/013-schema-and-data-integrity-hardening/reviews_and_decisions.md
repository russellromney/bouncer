# Reviews And Decisions

This file is append-only.

- Review rounds are written in Session B.
- Prefer a different model family from Session A when possible.
- Decision rounds are written in Session A in response to findings.
- Do not rewrite earlier review text to make it look resolved.

## Handoff Note

Phase 013 is ready for intent/plan review before implementation.

Suggested next session flow:

1. Read `../idd`.
2. Read `SYSTEM.md`, `ROADMAP.md`, this phase's `spec-diff.md`, and
   `plan.md`.
3. Review the spec diff and plan in this file.
4. Append review findings with stable IDs.
5. Let Session A respond with decisions before coding.

## Intent Review 1

Target:
- spec-diff review

Session:
- B

Model family:
- Claude Opus 4.7. Same family as the likely Session A author;
  cross-family reviewer was not available for this session.

Artifacts reviewed:
- `spec-diff.md`

Verification reviewed:
- not run (intent review only; hardening suite does not exist yet)
- read `SYSTEM.md` for the proved baseline (single
  `bouncer_resources` table, six columns, two CHECK constraints,
  `Error` enum with `Sqlite`, `NotInTransaction`, `InvalidTtlMs`,
  `TtlOverflow`, `TokenOverflow`, `InvalidLeaseRow`)
- read `ROADMAP.md` Phase 013 stub for original framing ("invalid/
  manual rows, schema drift, old schema versions, token near-
  overflow, bad ttl_ms, huge timestamps, unusual names/owners,
  partial application edits")
- read `bouncer-core/src/lib.rs` schema text (lines 102-119),
  `Error` enum (lines 18-32), `ResourceRow::validate` (lines
  87-98), `BOUNCER_SCHEMA_VERSION` constant (line 14), and
  existing overflow paths (`TokenOverflow` at line 225,
  `TtlOverflow` at line 466)

### Positive conformance review

- [P1] The default posture ("loud failure, not silent repair") is
  the right primitive stance. Bouncer's job is to refuse to make
  ambiguous data more ambiguous; the spec-diff commits to that
  explicitly. This is the correct contrast with what a higher-
  level framework would do (auto-migrate, coerce, retry).
- [P2] Folding "old schema version" into "schema drift" rather
  than launching a migration engine is a clean scope-fence and
  matches the ROADMAP framing ("Make impossible rows either
  impossible by constraint or loud by error"). The phase is about
  hardening the V1 contract, not designing V2.
- [P3] Excluding migration, automatic repair, and hidden
  normalization keeps Bouncer's "primitive, not framework"
  posture intact and matches Phase 011/012's discipline.
- [P4] Honoring "round-trip practical unusual values" without
  trimming or normalization is the right behavior for an opaque-
  TEXT column. SQLite round-trips bytes; Bouncer should not
  invent a layer that doesn't exist.

### Negative conformance review

- [N1] **"Closely enough for safe operation" silently delegates
  the integrity contract to implementation.** The spec-diff says
  bootstrap fails if a preexisting `bouncer_resources` table
  "does not match the current proved schema closely enough for
  safe operation." That phrase is doing real work: it picks the
  difference between "exact column list, types, NOT NULL, and
  both CHECKs" (strict) and "presence of required columns with
  compatible types" (permissive). Pin one. The actual contract
  surface is small (six columns plus two CHECKs); enumerate it in
  the spec-diff so the test surface is reviewable. Phase 012 did
  this for matrix material-row criteria ([D8]); same shape here.
- [N2] **Bug-fix escape clause language is inherited from 011/
  012 but Phase 013 makes it harder to apply.** Phase 013 *is*
  the phase that changes loud-failure semantics; nearly every
  shipped bullet is a behavior change. The 012 [D1] rule
  ("small direct fix, no new public type, schema shape, or
  documented semantic") needs sharpening: 013 will almost
  certainly add a new `Error` variant and may tighten the schema
  bootstrap signature. Decide: is "schema-mismatch error variant"
  in scope for this phase, or is it the kind of thing that
  triggers a split decision round? Without a sharper rule the
  phase will silently widen the public error surface.
- [N3] **Python test continuity isn't asserted.** Phase 011 [D9]
  / Phase 012 [D2] closed this. Phase 013 has higher exposure
  than either: a new schema-mismatch failure path will surface to
  Python as `BouncerError`. Worth a one-line acceptance like
  prior phases.
- [N4] **"Huge timestamps" from the ROADMAP stub is silently
  pruned.** ROADMAP line 98 lists "huge timestamps" alongside
  "token near-overflow" and "bad ttl_ms." The spec-diff folds
  this into `now_ms + ttl_ms` overflow only. Plausible silent
  drops include `now_ms < 0`, `now_ms` near `i64::MAX`, and
  stored `lease_expires_at_ms` beyond reasonable wall-clock
  dates. Either explicitly defer those or fold one sanity-check
  row in. Phase 012 had the same gap with `synchronous`/
  `locking_mode` ([A6]/[D7]); same fix applies here.

### Adversarial review

- [A1] **The current schema is six columns and two CHECKs; the
  spec-diff should pin which subset is the integrity contract.**
  The columns are `name`, `owner`, `token`, `lease_expires_at_ms`,
  `created_at_ms`, `updated_at_ms`. The CHECKs are `token >= 1`
  and the owner/expiry-pair invariant. Plausible integrity
  contracts:
  (a) **strict equality**: exact column list, types, NOT NULL,
      both CHECKs — any deviation fails. A future Bouncer that
      adds a column has to migrate.
  (b) **structural superset**: required columns present with
      matching types and CHECKs; extra columns are tolerated.
      Lets users add custom columns; tolerates future Bouncer
      adding columns to old DBs.
  (c) **column existence + load-bearing CHECKs**: required column
      names + types + the two CHECKs; ignore NOT NULL on
      `created_at_ms` / `updated_at_ms` etc. (loose).
  Each has different consequences for users running tools that
  add columns, and for future Bouncer versions. (a) is the most
  honest "loud failure" stance; (b) trades some loudness for
  forward-compat ergonomics. Pick before code; the choice is
  intent, not implementation.
- [A2] **`bootstrap_bouncer_schema(conn)` is the change site,
  and that's a behavior change to a public function.** Today
  bootstrap is `CREATE TABLE IF NOT EXISTS` and silently succeeds
  if a `bouncer_resources` table exists with any shape. After
  Phase 013, bootstrap fails on shape mismatch. Two pins needed:
  (a) Is the new check folded into `bootstrap_bouncer_schema`, or
      does Phase 013 add a separate `verify_bouncer_schema`
      helper that callers must opt into? Folding into bootstrap
      is the loud-by-default posture the spec-diff wants;
      separate helper preserves backward compat. Pick the
      bootstrap-folded path explicitly so callers know.
  (b) Idempotency under a *valid* existing Bouncer schema must
      still succeed. Pin: "calling bootstrap on a database that
      already has a current-shape `bouncer_resources` table is a
      no-op success."
- [A3] **`InvalidLeaseRow` and the `validate()` helper already
  exist in core (lines 31 and 87-98).** The spec-diff reads as if
  "fail loudly on invalid rows" is new behavior. It isn't — the
  mechanism exists. What 013 contributes is broader proof and
  possibly broader detection (e.g., checking row shape on every
  read path, not only after a SELECT). Make this clear so
  reviewers don't think 013 is adding a new failure mechanism it
  isn't, and so the spec-diff captures the *delta* (proof
  surface and possibly detection coverage).
- [A4] **Token near-overflow and TTL overflow already error
  loudly.** `Error::TokenOverflow` (line 29, raised at line 225)
  and `Error::TtlOverflow` (line 27, raised at line 466) are
  pre-existing. Same comment as [A3]: the spec-diff bullets read
  as new behavior; they are coverage broadening. Either rename
  the bullets ("preserve and prove" instead of "stays loud") or
  call out which overflow paths are *not* yet exercised.
- [A5] **"Practical unusual values" is undefined.** Pin a
  concrete list before code: 4-byte UTF-8 emoji, Unicode
  combining marks, RTL marks, zero-width joiners, leading/
  trailing whitespace, paths-like strings, SQL-shaped strings
  (e.g., `'; DROP TABLE bouncer_resources; --`), bytes-but-not-
  valid-UTF-8 if SQLite stores them, and a long-ish string (1KB?
  64KB?). The matrix's `Expect` enum approach from Phase 012
  ([A10]/[D3]) helps here: round-trip success vs explicit
  rejection, declared per row.
- [A6] **What about empty and NULL inputs?** `name TEXT PRIMARY
  KEY` rejects NULL by SQLite convention, but does Bouncer's API
  reject the empty string `""`? What about names containing the
  byte 0? What about owners that are `""`? The spec-diff promises
  no normalization, but it doesn't say whether empty/NUL inputs
  are accepted, rejected, or undefined. Pin.
- [A7] **"Partial application edits" is undefined.** Plausible
  interpretations:
  (a) row updated outside Bouncer with new owner but stale
      `lease_expires_at_ms`
  (b) row with `owner` set but `lease_expires_at_ms` NULL (CHECK
      already prevents this)
  (c) row with `token = 0` (CHECK already prevents this)
  (d) row missing `created_at_ms` or `updated_at_ms`
  (e) `owner = ""` with `lease_expires_at_ms = 0`
  Some of these are already prevented by SQLite CHECK constraints;
  some require new Rust-side validation. The spec-diff treats them
  as one bucket; the implementation will need to pick which class
  is the matrix. Pin which.
- [A8] **The spec-diff doesn't address whether Phase 013
  introduces a new public `Error` variant.** Today's enum has six
  variants; "schema integrity" is a new failure class. A new
  variant is a public API change. Three options:
  (a) add `Error::SchemaMismatch { reason: String }` — clean,
      breaks pattern-matching callers
  (b) reuse `Error::InvalidLeaseRow(String)` with a sentinel
      message — preserves variant set, awful for callers
  (c) bubble the schema-mismatch as `Error::Sqlite(...)` — wrong
      class, hides the diagnostic
  (a) is the correct answer; pin it now so the variant addition
   is the *intended* change, not silent drift discovered in
  implementation review.
- [A9] **`BOUNCER_SCHEMA_VERSION = "1"` is currently dead code.**
  It exists at lib.rs line 14 but isn't referenced anywhere
  (grep confirms). Phase 013 mentions "old schema version" as
  drift. Pin: does 013 give this constant teeth (e.g., persist it
  in `PRAGMA user_version` or a sibling table), or does it
  continue to be unused decoration with the version implicit in
  column/CHECK shape? Both are defensible; silence is not.
- [A10] **The "thin wrapper and SQL-extension coverage only
  where the surface behavior matters" line is undercommitted.**
  Phase 012's [D8] gave a clean rule for material rows. Reuse:
  wrapper/SQL coverage is in scope only where the surface
  exposes a *different* failure than core (e.g., the SQL UDF
  boundary swallowing schema-mismatch into `SQLITE_ERROR`).
  Otherwise core-only.
- [A11] **Concurrent bootstrap on a fresh DB.** Today bootstrap
  is `CREATE TABLE IF NOT EXISTS` and is naturally idempotent.
  With a strict integrity check, two connections calling
  bootstrap simultaneously on a fresh DB could race: first
  creates the table, second sees the *valid* schema and
  succeeds. That's fine for valid schemas, but the spec-diff
  should note that bootstrap-vs-bootstrap concurrency is not
  newly tested in 013 (lock contention is Phase 012's surface).
  Probably out of scope; worth a one-line note.

### Review verdict

The spec-diff captures the right primitive posture (loud failure
beats silent repair) and the right scope-fences (no migration, no
normalization, no binding expansion). The work is real and well-
framed.

Blocking-ish gaps before plan and code:

- [N1] / [A1] — pin the integrity contract precisely. Strict-
  equality, structural-superset, or column-existence-plus-CHECK
  is the load-bearing decision; pick before plan.
- [A2] — pin that the new check is folded into
  `bootstrap_bouncer_schema`, and that idempotency on a
  current-shape DB is preserved.
- [A8] — pin that a new `Error::SchemaMismatch` variant is the
  intended public API addition, not silent drift.

Smaller pins worth resolving before plan:

- [N2] — sharpen the bug-fix escape clause for a phase whose job
  is changing failure semantics.
- [N3] — Python continuity acceptance line.
- [N4] / [A8] — explicitly defer or include "huge timestamps"
  variants from the ROADMAP stub.
- [A3] / [A4] — clarify that `InvalidLeaseRow` and overflow
  errors already exist; 013 broadens proof, may broaden
  detection.
- [A5] / [A6] / [A7] — pin concrete lists for "unusual values"
  and "partial application edits"; pick rejection vs round-trip
  for empty/NUL.
- [A9] — give `BOUNCER_SCHEMA_VERSION` teeth or call it dead
  code and remove.
- [A10] — wrapper/SQL inclusion criterion mirrors Phase 012
  [D8].
- [A11] — bootstrap concurrency one-line note.

After Session A pins these in a Review Response, the spec-diff is
implementable. The framing is right; the precision is the work.

## Review Response 1

Responding to:
- Intent Review 1

Session:
- A

### Inputs

- [N1] integrity contract is too fuzzy
- [N2] bug-fix escape clause needs sharpening
- [N3] Python continuity acceptance line
- [N4] ROADMAP "huge timestamps" was silently pruned
- [A1] strict-vs-superset schema contract
- [A2] bootstrap is the enforcement point and must stay idempotent on
  valid schema
- [A3] / [A4] clarify delta versus existing loud-failure mechanisms
- [A5] / [A6] / [A7] pin unusual-value and partial-edit buckets
- [A8] `Error::SchemaMismatch` as an intended API addition
- [A9] `BOUNCER_SCHEMA_VERSION` dead-code decision
- [A10] wrapper/SQL inclusion rule
- [A11] bootstrap concurrency note

### Decisions

- [D1] Accept [N1] and [A1].
  Action: the spec now pins a strict schema-integrity contract for
  `bouncer_resources`: exact six-column shape, column order, `name`
  primary-key position, required nullability, and the two load-bearing
  invariants. Extra columns are schema drift in this phase.

- [D2] Accept [A2].
  Action: schema checking is folded into
  `bootstrap_bouncer_schema(conn)` itself, and bootstrap remains an
  idempotent no-op success on an already-valid current-shape schema.

- [D3] Accept [A8].
  Action: `Error::SchemaMismatch { reason: String }` is now pinned as an
  intended public API addition for this phase.

- [D4] Accept [N2].
  Action: the spec now says in-scope production changes are limited to
  the explicitly named integrity/failure surfaces plus small direct
  fixes needed to support them, while broader lease-semantic changes
  still split into a follow-up phase.

- [D5] Accept [N3].
  Action: acceptance and verification now say Python tests still pass if
  production code changes.

- [D6] Accept [N4].
  Action: the spec now explicitly narrows "huge timestamps" to overflow
  and near-boundary proof in this phase, while deferring broader
  timestamp-policy decisions for large but internally consistent values.

- [D7] Accept [A3] and [A4].
  Action: the spec now states plainly that `InvalidLeaseRow`,
  `InvalidTtlMs`, `TtlOverflow`, and `TokenOverflow` already exist, and
  that 013 broadens proof plus schema-drift detection rather than
  inventing those failure classes from scratch.

- [D8] Accept [A5], [A6], and [A7].
  Action: the spec now pins a concrete unusual-value set: empty string,
  whitespace, newline/tab, Unicode including combining-mark cases,
  punctuation/SQL-shaped strings, and a long-ish UTF-8 string.
  Embedded NUL is explicitly deferred. "Partial application edits" is
  now pinned to row-level invariant violations such as owner/expiry
  mismatch or `token <= 0`, while missing columns remains schema drift.

- [D9] Accept [A9].
  Action: 013 does not add version storage machinery. The proved schema
  version stays implicit in the exact schema shape, and the currently
  unreferenced `BOUNCER_SCHEMA_VERSION` constant may be removed as dead
  code during implementation.

- [D10] Accept [A10].
  Action: wrapper and SQL-surface rows are now explicitly in scope only
  where those surfaces expose a caller-visible failure different from
  core.

- [D11] Accept [A11].
  Action: the spec now notes that bootstrap-vs-bootstrap concurrency on
  a fresh valid DB is not a new proof target in this phase.

### Verification

- Updated `spec-diff.md` to pin the previously silent decisions.
- Updated `plan.md` to align with the pinned spec before plan review.
- No code changed.
- No tests run; this is an artifact response.

### Decision verdict

Phase 013 now has an implementable spec: strict schema-drift posture,
bootstrap-folded enforcement, a deliberate `SchemaMismatch` variant,
concrete unusual-value buckets, and explicit deferrals where the phase
is not trying to promise more than it can prove.

## Plan Review 1

Target:
- plan review

Session:
- B

Model family:
- Claude Opus 4.7. Same family as the likely Session A author;
  cross-family reviewer not available for this session.

Artifacts reviewed:
- `spec-diff.md`
- `plan.md`

Verification reviewed:
- not run (plan review only)

Summary pasted by user in chat and preserved here by Session A:

- [P5]–[P9] positive conformance confirm the response landed cleanly and
  the plan structure mirrors prior phases' discipline.
- [N5] CHANGELOG should flag strict bootstrap plus any
  `BOUNCER_SCHEMA_VERSION` removal as deliberate behavior changes.
- [N6] acknowledge wrapper error ripple from `SchemaMismatch`.
- [A12] / [A14] pin the schema-validation approach.
- [A13] pin exact-text versus affinity-tolerant type matching.
- [A15] pin the invalid-row fixture recipe.
- [A16] pin test file location to `bouncer-core/tests/integrity.rs`.
- [A17] pin `SchemaMismatch.reason` as diagnostic, not contract.
- [A18] note bootstrap-inside-caller-tx remains valid.
- [A19] pin a concrete byte budget for the long-ish UTF-8 string.
- [A20] note embedded-NUL behavior is current rusqlite/SQLite default,
  not Bouncer contract.
- [A21] / [N5] note closeout docs should flag strict bootstrap and dead
  constant removal as deliberate.
- [A22] add an acceptance row for bootstrap on valid schema plus
  existing live lease.

## Review Response 2

Responding to:
- Plan Review 1

Session:
- A

### Inputs

- [N5] / [A21] CHANGELOG rollout note
- [N6] wrapper error ripple acknowledgement
- [A12] / [A14] schema-validation helper approach
- [A13] exact-text versus affinity-tolerant type matching
- [A15] invalid-row fixture recipe
- [A16] test file location
- [A17] `SchemaMismatch.reason` contract status
- [A18] bootstrap inside caller-owned transaction
- [A19] concrete byte budget for the long-ish UTF-8 string
- [A20] embedded-NUL behavior note
- [A22] bootstrap idempotency with existing live lease

### Decisions

- [D12] Accept [A12] and [A14].
  Action: the plan now pins the helper design to
  `PRAGMA table_info(bouncer_resources)` for exact column metadata plus
  `sqlite_master.sql` inspection for the two load-bearing CHECK
  constraints, using simple canonical text matching rather than a full
  parser or behavior probes.

- [D13] Accept [A13].
  Action: declared column type matching is now exact text in this phase,
  not affinity-tolerant. `BIGINT NOT NULL` where Bouncer expects
  `INTEGER NOT NULL` is deliberate schema drift.

- [D14] Accept [A15].
  Action: the plan now pins the invalid-row fixture recipe: create a
  deliberately broken `bouncer_resources` table shape directly in the
  test database without the relevant CHECK, insert the invalid row, then
  exercise the public API. `writable_schema` stays out of scope.

- [D15] Accept [A16].
  Action: the plan now pins the core hardening file location to
  `bouncer-core/tests/integrity.rs`.

- [D16] Accept [A17].
  Action: the plan now states that `SchemaMismatch.reason` is a
  human-readable diagnostic, not a stable contract string.

- [D17] Accept [A18].
  Action: the plan now states that bootstrap inside a caller-owned
  transaction remains valid and schema validation is read-only.

- [D18] Accept [A19].
  Action: the "long-ish UTF-8 string" is now pinned to 4 KiB.

- [D19] Accept [A20].
  Action: the plan now states that embedded-NUL behavior is current
  rusqlite/SQLite default behavior and is not part of Bouncer's
  contract.

- [D20] Accept [A22].
  Action: acceptance now includes a row for bootstrap on valid schema
  plus existing live lease preserving observability.

- [D21] Accept [N5], [A21], and [N6].
  Action: the plan now explicitly notes the closeout CHANGELOG burden
  for strict bootstrap validation and any
  `BOUNCER_SCHEMA_VERSION` removal, and it also acknowledges the wrapper
  error ripple while expecting `#[from]` to keep it low-touch.

### Verification

- Updated `plan.md` to pin the previously silent implementation
  mechanics and rollout notes.
- No code changed.
- No tests run; this is an artifact response.

### Decision verdict

Phase 013's plan is now concrete enough to code against: the helper
mechanics, type strictness, invalid-row fixture recipe, test placement,
acceptance rows, and rollout/documentation notes are all pinned rather
than deferred to implementation time.

## Plan Review 1

Target:
- plan review against the post-decision `spec-diff.md` and `plan.md`

Session:
- B

Model family:
- Claude Opus 4.7. Same family as the likely Session A author;
  cross-family reviewer not available for this session.

Artifacts reviewed:
- `spec-diff.md` (post-Review-Response-1)
- `plan.md`

Verification reviewed:
- spot-checked spec-diff against [D1]–[D11]: every decision lands
  with concrete language (strict integrity contract enumerated at
  lines 27-37; bootstrap-folded check at 21-26; `SchemaMismatch`
  variant at 38-40; bug-fix scoping at 97-101; Python continuity
  at 117; huge-timestamp narrowing at 57-62; existing-loud-failure
  acknowledgment at 12-16; unusual-value list at 63-71 with NUL
  deferral at 72-73; `BOUNCER_SCHEMA_VERSION` cleanup at 45-47;
  wrapper/SQL inclusion rule at 80-82; bootstrap-concurrency note
  at 94-96)
- read existing `bouncer-core/src/lib.rs` schema text and
  `ResourceRow::validate()` for context on what the helper would
  need to inspect
- read existing test placement (`bouncer-core/tests/invariants.rs`,
  `bouncer-core/tests/sqlite_matrix.rs`) for placement consistency

### Positive conformance review

- [P5] **Decisions land cleanly.** Every [D1]–[D11] response has
  concrete spec-diff language. No silent drift. The integrity
  contract is enumerated, not gestured at.
- [P6] **Context section names current loud-failure pieces
  honestly.** Lines 21-26 of `plan.md` enumerate
  `CREATE TABLE IF NOT EXISTS`, the row-level CHECKs,
  `ResourceRow::validate()`, and the existing overflow errors.
  This pre-empts the "are we adding new mechanisms?" confusion
  that drove [A3]/[A4] in Intent Review 1.
- [P7] **Build order is concrete and slice-ordered.** Helper →
  variant → tests → invalid-state recipe → cleanup → optional
  wrapper. Each step is reviewable independently.
- [P8] **Traps catch the right anti-patterns.** "Do not silently
  repair," "do not let schema validation become an accidental
  migration engine," "do not tighten name/owner policy casually,"
  "do not bury schema drift inside generic SQLite errors." Every
  one of these is a real failure mode for this phase.
- [P9] **Phase decisions explicitly call out file-backed
  databases for schema-drift cases (line 90-91).** Mirrors Phase
  012's [D4] discipline; correct for cases where on-disk shape
  matters.

### Negative conformance review

- [N5] **Plan does not address rollout impact in CHANGELOG
  shape.** Folding strict schema validation into
  `bootstrap_bouncer_schema` is a behavior change that may break
  existing callers running against drifted databases. The spec is
  deliberate about this; the plan should note that the CHANGELOG
  entry needs to flag it as a behavior change, not just "added
  hardening."
- [N6] **"Files likely to change" omits `packages/bouncer`.**
  Lines 154-159 list only `bouncer-core` and the intent
  artifacts. But adding `Error::SchemaMismatch` to core means the
  wrapper's `bouncer::Error` (`From<core::Error>`) will need to
  carry the new variant cleanly. If the wrapper currently uses
  `#[from]` it'll just work; if it enumerates variants, it
  needs an update. Worth one line acknowledging the wrapper
  ripple (even if the answer is "no change needed because
  `#[from]` does it for us").

### Adversarial review

- [A12] **The schema-validation helper is the central
  implementation risk and the plan barely sizes it.** Lines
  99-102 describe it as "a small schema-validation helper in
  `bouncer-core` that inspects an existing `bouncer_resources`
  table." But the strict integrity contract per [D1] requires
  checking:
  - exact six columns, in order, with declared names
  - primary-key position on `name`
  - NOT NULL on `token`, `created_at_ms`, `updated_at_ms`
  - the two CHECK constraints (`token >= 1` and the owner/expiry
    pair invariant)

  These don't all live in the same SQLite metadata source.
  `PRAGMA table_info(bouncer_resources)` gives column name,
  declared type, NOT NULL, default, pk-position. CHECK
  constraints live in `sqlite_master.sql` as part of the
  CREATE TABLE text. Two plausible approaches:

  (a) **PRAGMA + sqlite_master.sql parsing.** Use
      `pragma_table_info` for columns and parse `sqlite_master.sql`
      to extract CHECKs. Parsing is fragile (whitespace, quoting,
      CHECK ordering).
  (b) **PRAGMA + canonical-DDL string compare.** Use
      `pragma_table_info` for columns; compare full
      `sqlite_master.sql` text against a canonical normalized
      string. Whitespace-sensitive; users with manually-formatted
      DDL fail.
  (c) **PRAGMA + behavior probe.** Use `pragma_table_info` for
      columns; insert/rollback probe rows that exercise each
      CHECK. Touches data path; needs SAVEPOINT discipline.

  Pin the approach. [D1] is strict so (a) or (c) is more honest;
  (b) is brittle. Without picking, implementation will silently
  drift toward whichever is easiest first.
- [A13] **`PRAGMA table_info` returns user-declared column type
  as text, not SQLite affinity class.** SQLite's type system is
  loose: `INTEGER`, `INT`, `BIGINT` all map to INTEGER affinity.
  Per [D1] the contract is "exact six columns, column order,
  declared column names, primary-key position on `name`, required
  nullability." Pin: does "declared column names" mean
  *exact column names* and tolerated-affinity types, or
  exact-text type matches too? A user who runs
  `CREATE TABLE bouncer_resources (... token BIGINT NOT NULL ...)`
  has the same affinity but different text — does that fail?
  Probably yes (strict posture), but spell it out so it's
  intentional.
- [A14] **CHECK constraint detection is not designed.** Plan
  doesn't pin how the two CHECKs are verified. This is harder
  than column metadata: `PRAGMA table_info` doesn't surface
  CHECKs at all. Without a chosen approach (parse, text-compare,
  probe), the implementation will pick at write time and the
  review will discover whatever it picked. The plan should
  commit to one — recommended: parse `sqlite_master.sql` for the
  table-level CHECK clauses with simple text matching against
  canonical forms, on the basis that the V1 schema is small and
  the CHECKs are short.
- [A15] **Constructing invalid persisted state for tests is
  non-trivial and the plan does not pin the recipe.** Plan line
  114-116 says "reuse direct table writes only to create invalid
  persisted state." But the row-level CHECKs already prevent
  several invalid shapes — `token = 0`, owner-without-expiry —
  through SQLite, so direct INSERTs fail. To exercise
  `Error::InvalidLeaseRow` against truly-invalid rows, the test
  has to either:
  (a) create the table without the CHECKs first, then insert,
      then ALTER (or accept the absence of CHECKs in the test
      fixture)
  (b) use `PRAGMA writable_schema = 1` to bypass constraints
      while inserting
  (c) create a deliberately-broken parallel table

  Pick (a) or (c); (b) is too cute. Pin the recipe so test
  authors don't reinvent it. Phase 012's matrix did this for
  `DbFile::fresh()`; same shape needed here for an
  `invalid_row_setup` helper.
- [A16] **Test file location is "likely
  `bouncer-core/tests/integrity.rs`" but not pinned.** Phase 011
  used `tests/invariants.rs`; Phase 012 used `tests/
  sqlite_matrix.rs`. Phase 013 should pin the same way before
  code. Recommended: `bouncer-core/tests/integrity.rs`. Same
  argument as [A18] in Phase 012's plan review.
- [A17] **`SchemaMismatch { reason: String }` reason-text
  contract is unspecified.** Phase 012's [D3] put exact error-
  message text outside the contract. The same posture should
  apply to `SchemaMismatch.reason`. Pin: callers may match on
  the variant; the `reason` field is human-readable diagnostic,
  subject to change across phases without a breaking-change
  bump. Otherwise, future implementations risk freezing the
  exact phrasing as load-bearing.
- [A18] **Bootstrap inside a caller-owned transaction is
  unaddressed.** Today `bootstrap_bouncer_schema(conn)` works
  inside or outside a caller-managed BEGIN. The new validation
  path queries `sqlite_master` and `pragma_table_info` — both
  work inside transactions, but the failure mode (returning
  `Err(SchemaMismatch)` while the caller's tx is still open)
  must be predictable. Probably this just works because
  validation is read-only, but worth a one-line note that
  bootstrap-inside-tx remains valid and validation does not
  pollute caller tx state.
- [A19] **"Long-ish UTF-8 string" needs a concrete byte budget.**
  Spec-diff line 71 / plan line 73 leave "long-ish" undefined.
  At write time the test author will pick something. Pin a
  representative number — e.g., 4 KiB — so the test row is a
  deliberate proof, not whatever felt readable. Mirrors Phase
  012's [A9] (cell list) and [A10] (`Expect` enum) discipline.
- [A20] **Embedded NUL deferral has an undefined behavior
  risk.** Spec-diff line 72-73 says NUL is "explicitly deferred;
  this phase does not add it to the proved contract." But
  rusqlite passes Rust strings through SQLite's
  `sqlite3_bind_text` which is NUL-terminated unless given a
  length. In practice this means a string `"a\0b"` is silently
  truncated to `"a"` on bind. "Deferred" without a behavior
  pin means a future user of this contract is left guessing
  whether NUL is rejected, truncated, or round-trips. Worth one
  line: "behavior of embedded NUL is currently
  rusqlite/SQLite's default (silent truncation at NUL) and is
  not part of Bouncer's contract."
- [A21] **`BOUNCER_SCHEMA_VERSION` removal is a public API
  removal.** Plan line 117 says "Remove `BOUNCER_SCHEMA_VERSION`
  if the phase still leaves it unused." This is a `pub const`
  removal — small, but technically an API change. Per [D4]'s
  bug-fix scoping ("small direct fixes needed to support
  [integrity] surfaces"), removing a dead constant counts as
  cleanup, not a lease-semantic change. Acceptable, but worth
  one CHANGELOG line at closeout to note the constant
  disappeared.
- [A22] **Bootstrap idempotency case for "valid schema with
  existing data rows" is implicit but not enumerated.** Plan
  acceptance line 124 says "Bootstrap remains idempotent on an
  already-valid current-shape schema." Spec-diff coverage at
  line 109 says "bootstrap idempotency on an already-valid
  current-shape schema." Both are silent on whether existing
  rows survive. They obviously should, but a deliberate test
  row covering "valid schema + valid live lease + bootstrap →
  lease still observable" closes the obvious bug class
  ("validation accidentally recreated the table"). Add one
  acceptance row.

### Review verdict

The plan is implementable in shape, but defers four real design
decisions to test-writing time:

- [A12] / [A14] — pin the schema-detection mechanism (PRAGMA
  table_info plus sqlite_master.sql parsing, vs alternatives),
  including how the two CHECK constraints are verified
- [A13] — pin whether type matches are exact-text or
  affinity-tolerant
- [A15] — pin the invalid-row test fixture recipe (drop-CHECKs
  table vs writable_schema vs parallel-broken-table)
- [A16] — pin test file location

Smaller pins worth resolving before code:

- [N5] / [A21] — CHANGELOG noting bootstrap is now strict and
  `BOUNCER_SCHEMA_VERSION` was removed
- [N6] — wrapper-error ripple acknowledged
- [A17] — `SchemaMismatch.reason` field is diagnostic, not
  contract
- [A18] — bootstrap-inside-caller-tx remains valid
- [A19] — concrete byte size for "long-ish" string
- [A20] — explicit note on rusqlite NUL truncation behavior
- [A22] — bootstrap-with-existing-rows idempotency row in
  acceptance

After Session A pins [A12]/[A14], [A13], [A15], and [A16] in a
Review Response, the plan is implementable. The other findings
can land as small follow-up commits inside this phase.

## Implementation Notes 1

Session:
- A

Artifacts changed:
- `bouncer-core/src/lib.rs`
- `bouncer-core/tests/integrity.rs` (new)

### Implemented

- Added `Error::SchemaMismatch { reason: String }` as a deliberate
  public variant, per [D3]. The `reason` is human-readable
  diagnostic only ([D16]); callers may match on the variant.
- Folded strict schema validation into
  `bootstrap_bouncer_schema(conn)` per [D2]: when the
  `bouncer_resources` table is absent, the canonical CREATE runs;
  when present, validation runs and bootstrap stays a no-op
  success on a current-shape schema.
- Validation uses `PRAGMA table_info(bouncer_resources)` for
  exact column metadata (cid, name, declared type, NOT NULL,
  primary-key position) plus a normalized substring match over
  `sqlite_master.sql` for the two load-bearing CHECK constraints,
  per [D12]. No behavior probes, no full SQL parser.
- Declared type matching is exact text per [D13]:
  `INTEGER NOT NULL` is required where the schema declares
  `INTEGER NOT NULL`. `BIGINT NOT NULL` (same affinity, different
  declared text) is rejected with a diagnostic reason.
- Removed the unreferenced `pub const BOUNCER_SCHEMA_VERSION`
  per [D9] and plan step 6. No remaining grep hits in src/, tests/,
  or bindings.
- Wrapper ripple: `bouncer::Error` already carries
  `core::Error` via `#[from]`; no wrapper code change was needed.
  `make test-rust` confirms the wrapper crate builds and all
  wrapper tests still pass.
- Python ripple: `BouncerError` is constructed via
  `err.to_string()`, so `Error::SchemaMismatch` flows to Python
  through Display without binding-side change. `make test`
  Python suite (20 tests) still passes.
- Added `bouncer-core/tests/integrity.rs` (32 tests) per [D15],
  with each test on a fresh tempdir/database file per [D4]'s
  isolation pattern.

### Test coverage

Schema-validation positive:
- `bootstrap_creates_fresh_schema_when_table_absent`
- `bootstrap_is_idempotent_on_valid_schema`
- `bootstrap_preserves_existing_live_lease` ([D20] / [A22])
- `bootstrap_inside_caller_owned_transaction_is_valid` ([D17])

Schema-validation negative (each via a deliberately broken
`CREATE TABLE` rather than `writable_schema`, per [D14]):
- `bootstrap_rejects_table_with_extra_column`
- `bootstrap_rejects_table_missing_required_column`
- `bootstrap_rejects_table_with_swapped_column_order`
- `bootstrap_rejects_affinity_compatible_but_text_different_type`
  (BIGINT vs INTEGER, [D13])
- `bootstrap_rejects_table_with_wrong_nullability`
- `bootstrap_rejects_table_with_wrong_primary_key`
- `bootstrap_rejects_table_without_token_check`
- `bootstrap_rejects_table_without_pair_check`

Invalid-row content (using a fresh `bouncer_resources` table
without the row-level CHECKs so the bad rows can be persisted, and
hitting the public API directly so `ResourceRow::validate` is the
detection boundary):
- `inspect_rejects_row_with_owner_but_no_expiry`
- `inspect_rejects_row_with_expiry_but_no_owner`
- `inspect_rejects_row_with_zero_token`
- `claim_rejects_row_with_invalid_state`
- `renew_rejects_row_with_invalid_state`
- `release_rejects_row_with_invalid_state`

Token near-overflow (asserts non-mutating):
- `claim_takeover_fails_at_token_max_without_mutating_row`
- `claim_takeover_at_token_max_minus_one_succeeds_to_max`

TTL edges (asserts non-mutating where applicable):
- `claim_rejects_zero_ttl`
- `claim_rejects_negative_ttl`
- `renew_rejects_zero_ttl`
- `renew_rejects_negative_ttl`
- `claim_rejects_ttl_overflow_at_i64_boundary`
- `renew_rejects_ttl_overflow_at_i64_boundary`

Unusual text round-trip (each runs claim → inspect → owner →
token → renew → release → reclaim, asserting opaque storage and
token monotonicity):
- `round_trip_empty_strings`
- `round_trip_whitespace_only_strings`
- `round_trip_strings_with_newlines_and_tabs`
- `round_trip_unicode_and_combining_marks`
- `round_trip_punctuation_and_sql_shaped_strings`
- `round_trip_long_4kib_utf8_string` (4 KiB exact, mixed
  ASCII + multi-byte UTF-8 per [D18])

### Decisions honored

- [D1] strict integrity contract — six columns enumerated with
  `cid`, declared type, NOT NULL, and PK position per
  `EXPECTED_COLUMNS` in `lib.rs`
- [D2] bootstrap-folded enforcement plus idempotency on valid
  schema
- [D3] `Error::SchemaMismatch { reason }` variant added
- [D4] (012) bug-fix scoping: the only production-code changes
  are the new variant, the validation helper, and removal of an
  unreferenced constant — no public type rename, no schema-shape
  change beyond the deliberate strict contract
- [D5] Python tests pass (`make test` ran clean)
- [D6] huge-timestamp narrowing: `claim_rejects_ttl_overflow_at_
  i64_boundary` and `renew_rejects_ttl_overflow_at_i64_boundary`
  prove only the `now_ms + ttl_ms` overflow path, no broader
  timestamp policy
- [D7] `InvalidLeaseRow` and overflow errors are exercised
  through public surfaces; mechanism was pre-existing
- [D8] unusual-value bucket pinned per the spec list; embedded
  NUL deferred (no test exercises it)
- [D9] `BOUNCER_SCHEMA_VERSION` removed
- [D10] no wrapper or SQL-extension test rows added because
  neither surface exposes a different caller-visible failure for
  these cases
- [D11] no concurrent-bootstrap tests added
- [D12] / [D14] schema-validation helper uses
  `pragma_table_info` + canonical text matching of CHECK
  fragments; invalid-row fixture uses a deliberately broken table
  shape, no `writable_schema`
- [D13] declared types matched by exact text
- [D15] tests landed at `bouncer-core/tests/integrity.rs`
- [D16] `SchemaMismatch.reason` is diagnostic; tests assert
  substring snippets only, never exact text
- [D17] bootstrap inside a caller-owned tx is read-only and
  proven by `bootstrap_inside_caller_owned_transaction_is_valid`
- [D18] long-ish string is 4 KiB
- [D19] embedded NUL not asserted; remains rusqlite/SQLite default
- [D20] bootstrap-with-existing-lease idempotency proven
- [D21] CHANGELOG burden noted for closeout (strict bootstrap
  validation + `BOUNCER_SCHEMA_VERSION` removal); not yet
  written, pending implementation review

### No semantic surprises

Implementation did not surface a real core bug. Every test passed
on first run after the validation helper compiled cleanly. No
production lease semantics changed beyond the deliberate strict
schema-drift posture. The only existing-test impact was confirming
that the 27 prior `bouncer-core` lib tests, the 2 invariants, and
the 13 + 3 + 5 + 1 matrix/cdylib tests still pass unchanged.

### Verification

- `cargo build -p bouncer-core` — clean
- `cargo test -p bouncer-core --lib` — 27 passed
- `cargo test -p bouncer-core --test integrity` — 32 passed in
  ~30 ms
- `cargo test -p bouncer-core --test invariants` — 2 passed
  (1000 × 100 generated runner unchanged)
- `cargo test -p bouncer-core --test sqlite_matrix` — 15 passed
- `make test-rust` — full Rust suite green (35 + 1 + 5 + 27 +
  32 + 2 + 15 across the test binaries)
- `make test` — Rust + Python green (20 pytest tests pass)

### Closeout status

- `SYSTEM.md` was intentionally not updated; per IDD and the phase
  instruction it should wait until implementation review and
  acceptance.
- `CHANGELOG.md`, `ROADMAP.md`, and `commits.txt` were not updated
  for the same review-gated reason. The CHANGELOG entry, when
  written, must explicitly flag (a) `bootstrap_bouncer_schema` is
  now strict against drifted persisted state, (b)
  `Error::SchemaMismatch` is a new public variant, and (c)
  `BOUNCER_SCHEMA_VERSION` was removed as dead code per [D21].

## Implementation Review 1

Target:
- implementation review

Session:
- B

Model family:
- GPT-5

Artifacts reviewed:
- `bouncer-core/src/lib.rs`
- `bouncer-core/tests/integrity.rs`
- `packages/bouncer/src/lib.rs`
- `.intent/phases/013-schema-and-data-integrity-hardening/spec-diff.md`
- `.intent/phases/013-schema-and-data-integrity-hardening/plan.md`
- `.intent/phases/013-schema-and-data-integrity-hardening/reviews_and_decisions.md`

Verification reviewed:
- `cargo test -p bouncer-core --test integrity`
- `cargo test -p bouncer-core`
- `cargo test -p bouncer`
- `make test-rust`
- `make test-python`

### Findings

No findings.

### Positive conformance review

- [P10] The implementation matches the pinned phase shape. Strict
  schema validation is folded into `bootstrap_bouncer_schema`
  rather than split behind a second API, and valid existing schema
  remains an idempotent no-op success.
- [P11] `Error::SchemaMismatch { reason }` landed as an explicit
  public variant, and the code treats `reason` as diagnostic text
  rather than a caller contract. That matches the 013 decisions
  and keeps the failure class cleanly separated from
  `InvalidLeaseRow` and incidental SQLite errors.
- [P12] The validation mechanism is the one the plan pinned:
  `PRAGMA table_info('bouncer_resources')` for exact column
  metadata plus `sqlite_master.sql` fragment checks for the two
  load-bearing CHECK constraints. No migration behavior or hidden
  repair crept in.
- [P13] The hardening suite covers the right persisted-state
  failure buckets directly in `bouncer-core/tests/integrity.rs`:
  schema drift, invalid manual rows, TTL edges, token overflow,
  unusual text round-trip, bootstrap idempotency, and
  bootstrap-with-existing-live-lease.
- [P14] The invalid-row fixture recipe follows the reviewed plan:
  bad persisted rows are created through a deliberately weakened
  table shape, not `writable_schema`, so the proof remains
  understandable and file-backed.
- [P15] Wrapper ripple stayed boring in the good way. The wrapper
  crate needed no semantic change beyond existing `#[from]`
  propagation, and `cargo test -p bouncer` stayed green. That
  matches the "core-first unless a surface diverges" rule.
- [P16] The closeout-sensitive behavior changes are real and
  visible, not accidental drift: bootstrap is now strict against
  drifted schema, `SchemaMismatch` is part of the public error
  surface, and `BOUNCER_SCHEMA_VERSION` is gone as dead code.

### Residual risk

The one brittle spot is already an intentional phase decision, not
an implementation defect: CHECK validation depends on normalized
`sqlite_master.sql` text rather than a SQL parser. That means any
future intentional schema edit should be treated as a schema-
contract change and reviewed as such.

### Review verdict

Accepted.

The implementation matches the pinned 013 contract and I did not
find a semantic bug or a spec/plan contradiction in the landed
code. It is ready for closeout updates (`SYSTEM.md`,
`CHANGELOG.md`, `ROADMAP.md`, and `commits.txt`) and then
commit/push.

## Implementation Closeout 1

Session:
- A

Closeout work completed:

- updated `SYSTEM.md` to fold Phase 013 into the proved baseline:
  strict bootstrap validation, `SchemaMismatch`, and loud invalid-row
  behavior are now part of the current system model
- updated `CHANGELOG.md` to record the deliberate behavior changes:
  strict bootstrap, new public `SchemaMismatch` variant, and dead
  `BOUNCER_SCHEMA_VERSION` removal
- updated `ROADMAP.md` to move Phase 013 out of "next build steps" and
  mark the hardening phase as landed in current status
- prepared `commits.txt` for the landing commit receipt

Verification carried forward:

- `cargo test -p bouncer-core --test integrity`
- `cargo test -p bouncer-core`
- `cargo test -p bouncer`
- `make test-rust`
- `make test-python`

Closeout note:

Phase 013 is accepted and closed at the artifact/doc level. The next
phase to plan is Phase 014 — docs as safety rails.
