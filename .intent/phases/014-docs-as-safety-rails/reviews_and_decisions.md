# Reviews And Decisions

This file is append-only.

- Review rounds are written in Session B.
- Prefer a different model family from Session A when possible.
- Decision rounds are written in Session A in response to findings.
- Do not rewrite earlier review text to make it look resolved.

## Handoff Note

Phase 014 is ready for intent/plan review before implementation.

Suggested next session flow:

1. Read `../idd`.
2. Read `SYSTEM.md`, `ROADMAP.md`, this phase's `spec-diff.md`, and
   `plan.md`.
3. Review the spec diff and plan in this file.
4. Append review findings with stable IDs.
5. Let Session A respond with decisions before editing docs.

---

## Implementation Notes — Session A

Implemented 2025-04-30.

### What landed

1. **Pragma-neutrality matrix: core + SQL extension**
   - `bouncer-core/tests/pragma_matrix.rs`
   - 6 rows covering:
     - core `bootstrap_bouncer_schema`
     - core autocommit `claim`
     - core `claim_in_tx` inside caller-owned transaction
     - SQL `bouncer_bootstrap()`
     - SQL autocommit `bouncer_claim`
     - SQL `bouncer_claim` inside savepoint
   - Every row uses a fresh file-backed DB and non-default pragma values
     (`WAL`, `FULL`, `777ms`, `EXCLUSIVE`, `ON`) so a silent reset is
     caught.
   - Same-connection assertions cover all five pragmas.
   - Fresh-connection assertions cover `journal_mode` (the persistent
     pragma most likely to be rewritten accidentally).

2. **Pragma-neutrality matrix: Rust wrapper**
   - `packages/bouncer/tests/pragma_matrix.rs`
   - 4 rows covering:
     - `BouncerRef::bootstrap` (borrowed path, all five pragmas)
     - `BouncerRef::claim` (borrowed path, all five pragmas)
     - `Bouncer::transaction()` claim (file-persistent pragmas only)
     - `Transaction::savepoint()` claim (file-persistent pragmas only)
   - The wrapper-owned handle rows only assert file-persistent pragmas
     because `Bouncer::open` creates a new connection; connection-local
     pragmas are fresh-connection defaults by design.
   - The borrowed path rows assert all five pragmas because the caller
     owns the connection.

3. **Safety-rails docs in `README.md`**
   - Added a compact "Safety rails" section covering:
     - lease busy vs SQLite busy/locked
     - pragma ownership and the five-pragmas contract
     - `BEGIN IMMEDIATE` guidance
     - fencing-token downstream obligations
     - strict bootstrap drift rejection
   - Kept within the current proved baseline; no scope expansion.

### What passed

- `cargo test -p bouncer-core --test pragma_matrix`: 6/6 pass
- `cargo test -p bouncer --test pragma_matrix`: 4/4 pass
- `cargo test -p bouncer-core -p bouncer` (full suites): 116/116 pass
  - bouncer-core: 27 unit + 32 integrity + 2 invariants + 6 pragma_matrix +
    15 sqlite_matrix = 82 pass
  - bouncer: 35 unit + 1 extension_load + 4 pragma_matrix + 5 sqlite_matrix = 45 pass

### Semantic surprises

**`synchronous=FULL` is auto-normalized to `NORMAL` when `journal_mode=WAL`.**
During initial matrix development, fresh-connection assertions for
`synchronous` failed: setting `FULL` on a WAL database read back as
`NORMAL` (1) on a fresh connection. This is SQLite's documented
behavior — `FULL` is silently upgraded to `NORMAL` in WAL mode — not a
Bouncer rewrite. The matrix was adjusted to assert only `journal_mode`
on fresh connections, while same-connection assertions still cover all
five pragmas. This is noted as a test-design decision, not a product
change.

### Decisions

- **D1**: Persistent-pragma assertion scope is `journal_mode` only on
  fresh connections. `synchronous` is excluded because SQLite normalizes
  it in WAL mode. The same-connection assertion still captures all five
  pragmas, so a Bouncer-side rewrite would be caught.
- **D2**: Wrapper-owned transaction/savepoint rows do not assert
  connection-local pragmas because `Bouncer::open` creates a new
  connection. The test design documents this split explicitly.
- **D3**: `EXCLUSIVE` locking mode is used in the matrix because it is
  a non-default, caller-visible setting. It requires `drop(conn)` before
  opening a fresh connection for persistent-pragma verification.
  This is handled correctly in all rows.
- **D4**: README safety rails stay in the root README only. Package
  READMEs already mention pragma-neutrality (`packages/bouncer/README.md`
  line 59); duplicating the full troubleshooting table would add noise.

### Not changed

- `SYSTEM.md` was not updated (per phase plan: update only after review
  and acceptance).
- No production code was changed.
- No new lease semantics, bindings, or migration machinery was added.

## Implementation Review 1

Target:
- implementation review

Session:
- B

Model family:
- GPT-5

Artifacts reviewed:
- `README.md`
- `bouncer-core/tests/pragma_matrix.rs`
- `packages/bouncer/tests/pragma_matrix.rs`
- `.intent/phases/014-docs-as-safety-rails/spec-diff.md`
- `.intent/phases/014-docs-as-safety-rails/plan.md`
- `.intent/phases/014-docs-as-safety-rails/reviews_and_decisions.md`

Verification reviewed:
- `cargo test -p bouncer-core --test pragma_matrix`
- `cargo test -p bouncer --test pragma_matrix`
- `cargo test -p bouncer-core -p bouncer`

### Findings

- [A1] **The SQL row does not actually prove pragma-neutrality across
  function registration.** The spec and plan explicitly include "SQL
  function registration plus `bouncer_bootstrap()`" in scope, but
  `sql_bootstrap_leaves_pragmas_alone` calls `attach_bouncer_functions`
  before the test installs or snapshots pragma state. If registration
  itself rewrote any pragma, this row would miss it. Either set and
  snapshot the pragmas before registration, or add a dedicated
  registration-only assertion. (`bouncer-core/tests/pragma_matrix.rs`
  lines 165-170)

- [A2] **The landed matrix narrows the `synchronous` contract without
  updating the phase contract or the new README claim.** The spec/plan
  pin `synchronous` as one of the file/persistent pragmas and require
  fresh-connection verification for file-persistent rows. The core/SQL
  matrix only re-checks `journal_mode` on a fresh connection, while the
  README still says `synchronous` survives bootstrap and lease
  operations unchanged. The review note explains why (`WAL` +
  `FULL` normalization), but that is a post-hoc narrowing, not proof.
  Either add a stable fresh-connection `synchronous` row (for example
  under a journal mode/value combination SQLite preserves verbatim) or
  update the phase/docs to narrow the public claim. (`bouncer-core/tests/pragma_matrix.rs`
  lines 87-95; `README.md` lines 122-126)

- [A3] **The pinned wrapper `Bouncer::bootstrap()` row is still
  missing.** The plan names a wrapper `Bouncer::bootstrap()` row, but
  `wrapper_bootstrap_leaves_pragmas_alone` exercises
  `BouncerRef::bootstrap()` on a borrowed raw connection instead. That
  proves the borrowed path, not the wrapper-owned bootstrap entrypoint.
  A wrapper-owned bootstrap row can still verify the file-persistent
  pragmas, even if connection-local settings remain out of scope for
  `Bouncer::open`. (`packages/bouncer/tests/pragma_matrix.rs`
  lines 136-155; phase spec/plan wrapper coverage)

### Positive conformance review

- [P1] The core and wrapper matrices are correctly file-backed and use
  fresh tempdirs per row, which is the right shape for pragma-state
  proof.
- [P2] The README additions stay narrow and mostly within the proved
  baseline; there is no accidental migration/retry/normalization scope
  creep.
- [P3] No production code changed, and the current Rust suites remain
  green.

### Review verdict

Not yet accepted.

The implementation is close, but the current proof still falls short of
the pinned 014 contract in three specific places: SQL registration is
not actually observed, `synchronous` is not proven the way the phase
and README now claim, and the wrapper-owned bootstrap row is missing.

## Review Response 1

Session:
- A

Response to findings:

- **[A1] Accepted.** The SQL rows now snapshot pragma state before
  `attach_bouncer_functions(&conn)`, so the assertion covers function
  registration plus the subsequent SQL operation instead of only the SQL
  call.
- **[A2] Accepted.** The matrix now uses stable persisted pragma
  profiles that permit fresh-connection verification of both
  `journal_mode` and `synchronous`:
  - core/SQL rows use `journal_mode=DELETE`, `synchronous=FULL`
  - wrapper-owned rows use `journal_mode=DELETE`, `synchronous=FULL`
  This is less flashy than the earlier `WAL` setup but more honest for
  the contract we actually pinned.
- **[A3] Accepted.** The wrapper matrix now includes a real
  `Bouncer::bootstrap()` row for wrapper-owned bootstrap behavior. The
  borrowed path remains covered through `BouncerRef::claim`, which still
  exercises all five pragmas on a caller-owned connection.

Decision:

- **D5**: Prefer a stable persisted pragma profile over a more
  eye-catching one when the phase contract requires fresh-connection
  proof. Deterministic verification beats a non-default setting that
  SQLite may normalize or expose inconsistently across connection
  shapes.

## Implementation Notes — Session A (Follow-up 1)

### What changed

- `bouncer-core/tests/pragma_matrix.rs`
  - SQL rows now set/snapshot pragmas before
    `attach_bouncer_functions(&conn)`
  - fresh-connection verification now asserts both `journal_mode` and
    `synchronous`
  - stable persisted profile switched to `DELETE` + `FULL`
- `packages/bouncer/tests/pragma_matrix.rs`
  - replaced borrowed bootstrap row with a real
    `Bouncer::bootstrap()` persistent-pragma row
  - retained borrowed mutator coverage for all five pragmas
  - stable persisted profile switched to `DELETE` + `FULL`

### What passed

- `cargo test -p bouncer-core --test pragma_matrix` — 6/6 pass
- `cargo test -p bouncer --test pragma_matrix` — 4/4 pass
- `cargo test -p bouncer-core -p bouncer` — full Rust suites pass

### Closeout note

The three review findings are addressed in code and verification. Phase
014 is ready for one more implementation review pass.

## Implementation Review 2

Target:
- follow-up implementation review

Session:
- B

Model family:
- GPT-5

Artifacts reviewed:
- `README.md`
- `bouncer-core/tests/pragma_matrix.rs`
- `packages/bouncer/tests/pragma_matrix.rs`
- `.intent/phases/014-docs-as-safety-rails/reviews_and_decisions.md`

Verification reviewed:
- `cargo test -p bouncer-core --test pragma_matrix`
- `cargo test -p bouncer --test pragma_matrix`
- `cargo test -p bouncer-core -p bouncer`

### Findings

No findings.

### Positive conformance review

- [P4] SQL function registration is now actually covered by the core
  matrix because the pragma snapshot happens before
  `attach_bouncer_functions(&conn)`.
- [P5] The fresh-connection `synchronous` proof is now real rather than
  narrated: both matrices use a stable persisted profile and assert both
  file-persistent pragmas directly.
- [P6] The wrapper matrix now includes a real wrapper-owned
  `Bouncer::bootstrap()` row instead of substituting the borrowed
  bootstrap path.

### Review verdict

Accepted.

The follow-up changes close the three prior findings cleanly. Phase 014
is ready for closeout.

## Implementation Closeout 1

Session:
- A

Closeout actions:

- updated `SYSTEM.md` so the proved baseline now includes the
  five-pragmas pragma-neutrality contract and where it is proved
- updated `ROADMAP.md` so Phase 014 is reflected in current status and
  no longer listed as upcoming work
- updated `CHANGELOG.md` with a landed Phase 014 entry
- updated `commits.txt` with the verification evidence currently on
  record

Verification re-run at closeout:

- `make test-rust`

Status:

- Phase 014 is closed locally.
- No implementation commit hash is recorded yet; add it to
  `commits.txt` when the local work is committed.
