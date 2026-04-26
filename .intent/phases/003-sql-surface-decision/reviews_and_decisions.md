# Reviews And Decisions

This file is append-only.

- Review rounds are written in Session B.
- Decision rounds are written in Session A in response to findings.
- Do not rewrite earlier review text to make it look resolved.

## Decision Round 001

### Responding to

- direct human instruction that Phase 003 should stop pretending to be a
  decision phase and should instead move forward with the actual product
  decision

### Decisions

- [D1] Phase 003 is now an implementation phase, not a meta phase about
  whether to decide.
  Target:
  - `spec-diff.md`
  - `plan.md`

- [D2] Bouncer should ship a SQLite loadable-extension surface next.
  Rationale:
  - Bouncer is a SQLite coordination primitive.
  - SQL is the cleanest cross-language boundary for the single-machine
    Honker stack.
  - Honker already proves the family shape: core semantics in Rust,
    loadable extension as the shared public boundary, thin language
    bindings on top.
  Target:
  - `spec-diff.md`
  - `plan.md`
  - `ROADMAP.md`
  - `README.md`

- [D3] The first SQL surface keeps bootstrap explicit and time explicit.
  There are no implicit `now()` helpers in Phase 003.
  Target:
  - `spec-diff.md`
  - `plan.md`

- [D4] The minimum Phase 003 SQL contract is:
  - `bouncer_bootstrap()`
  - `bouncer_claim(name, owner, ttl_ms, now_ms)`
  - `bouncer_renew(name, owner, ttl_ms, now_ms)`
  - `bouncer_release(name, owner, now_ms)`
  - `bouncer_owner(name, now_ms)`
  - `bouncer_token(name)`
  Target:
  - `spec-diff.md`
  - `plan.md`
  - `README.md`

- [D5] The likely packaging shape is a new workspace crate for the
  loadable extension, following Honker's existing architecture rather
  than inventing a special-case path for Bouncer.
  Target:
  - `plan.md`

### Verdict

The human product decision is made. Phase 003 should now be reviewed and
implemented as the first SQL/loadable-extension phase, not revisited as
an open question.

## Review Round 001

Target:
- implementation review (Phase 003 SQL/loadable-extension surface)

Session:
- B

Model family:
- Claude Opus 4.7

Artifacts reviewed:
- `spec-diff.md`
- `plan.md`
- workspace `Cargo.toml`
- `bouncer-extension/Cargo.toml`
- `bouncer-extension/src/lib.rs`
- `bouncer-extension/README.md`
- `bouncer-honker/Cargo.toml`
- `bouncer-honker/src/lib.rs` (`attach_bouncer_functions`,
  `owner`, `token`, `to_sql_err`, plus the new
  `attached_sql_functions_cover_bootstrap_and_full_lease_cycle`
  test)
- `packages/bouncer/src/lib.rs` (new SQL test helpers + 6 new
  SQL/Rust interop tests)
- `README.md`, `ROADMAP.md`
- `commits.txt`

Verification reviewed:
- `cargo test` from repo root: 14 wrapper tests pass, 17 core
  tests pass (default-members excludes `bouncer-extension` —
  see [P5])
- `cargo build -p bouncer-extension`: builds cleanly
- `target/debug/libbouncer_ext.dylib` produced
- `cargo clippy --workspace --all-targets`: one warning, see
  [N1]
- one-off bundled-SQLite Rust probe (recorded in `commits.txt`)
  loaded the dylib and called `bouncer_bootstrap`,
  `bouncer_claim`, `bouncer_owner`, and `bouncer_token`
  successfully

### Positive conformance review

- [P1] Every spec-diff verification item has a named test or
  observable result:
  - "extension builds and loads" → builds, dylib produced, the
    user's manual probe loaded it
  - "`bouncer_bootstrap()` is explicit and idempotent" →
    `sql_bootstrap_is_explicit_and_idempotent` and
    `sql_functions_require_explicit_bootstrap`
  - "SQL `claim`/`renew`/`release` reuse core semantics" → the
    SQL functions in `attach_bouncer_functions` literally call
    `claim`/`renew`/`release`/`owner`/`token` from the same
    crate; `bouncer-extension/src/lib.rs` is 31 lines and
    contains zero lease logic
  - "file-backed interop tests" →
    `sql_claim_is_visible_to_wrapper_on_separate_connection`,
    `wrapper_claim_is_visible_to_sql_on_separate_connection`,
    `sql_and_rust_preserve_monotonic_fencing_tokens`
  - "explicit `now_ms` everywhere" → every SQL signature
    carries `now_ms` where time matters; no implicit clock read
- [P2] Layering is clean and slightly better than the plan
  required. `attach_bouncer_functions` lives in `bouncer-honker`
  rather than in `bouncer-extension`, so the SQL function
  definitions sit next to the lease semantics they delegate to,
  and any in-process Rust caller (the wrapper, an embedder)
  can install the same SQL surface without depending on
  `bouncer-extension`. The extension crate becomes a 13-line
  FFI shim. That is exactly the "core + extension + thin
  bindings" shape the plan called for.
- [P3] Zero SQL-side reimplementation of lease semantics. Every
  scalar function is a one-shot delegation to a `bouncer-honker`
  function. Result mapping (Acquired→token, Busy→NULL,
  Released→1, Rejected→0) matches the spec-diff "Expected
  return shape" section item-for-item. The asymmetry (some
  return tokens, `release` returns 0/1) reflects what each
  caller actually needs, not historical drift.
- [P4] `now_ms` stays explicit on every SQL signature that
  cares about liveness (`claim`, `renew`, `release`, `owner`).
  `bouncer_token(name)` correctly omits `now_ms` because token
  is a per-row constant readable across all liveness states.
  The SQL surface does not smuggle wall-clock ordering into
  the contract — matches [D3] and the deterministic-simulation
  direction in `ROADMAP.md`.
- [P5] `default-members = ["bouncer-honker", "packages/bouncer"]`
  excludes `bouncer-extension` from the default test graph.
  This is the correct workaround for rusqlite's
  `loadable_extension` feature being incompatible with normal
  test mode (the feature changes how rusqlite links against
  SQLite, breaking unit tests in the same default cargo
  invocation). The exclusion is documented with rationale in
  `commits.txt`. Pragmatic and well-named.
- [P6] `bouncer-honker/Cargo.toml` correctly enables the
  `functions` rusqlite feature, so `attach_bouncer_functions`
  compiles for any consumer (wrapper, extension, or future
  embedder) without needing to add the feature themselves.
- [P7] Documentation moved from "hypothetical" to "exists":
  README, ROADMAP, and `bouncer-extension/README.md` all list
  the SQL surface as a present capability. `SYSTEM.md` and
  `CHANGELOG.md` correctly **not** updated yet — that's the
  right IDD discipline (baseline updates after acceptance).
  `commits.txt` calls this out plainly.
- [P8] The new core test
  `attached_sql_functions_cover_bootstrap_and_full_lease_cycle`
  exercises the full SQL surface end-to-end inside a single
  test (bootstrap → claim → owner → token → renew → release →
  owner-after-release → token-after-release) with explicit
  return-shape assertions. This is the canonical "the SQL
  contract works" test and it lives in the right crate.

### Negative conformance review

- [N1] `bouncer-honker/src/lib.rs:329-334` triggers a clippy
  warning: `rusqlite::Error::UserFunctionError(Box::new(std::io::Error::new(std::io::ErrorKind::Other, err.to_string())))`
  should use `std::io::Error::other(err.to_string())`. Trivial
  fix, one line.
- [N2] No automated test proves the dylib actually loads via
  `SELECT load_extension('...')`. The wrapper interop tests
  call `attach_bouncer_functions` directly on a Rust-owned
  Connection, bypassing the dylib loading mechanism entirely.
  The dylib build is verified by `cargo build`, and the user's
  manual one-off probe (recorded in `commits.txt`) loaded it
  once — but a future change to the entrypoint name
  (`sqlite3_bouncerext_init`) or to the FFI shape would
  silently regress this without any test catching it. A small
  integration test that uses rusqlite's `load_extension`
  feature to load `target/debug/libbouncer_ext.dylib` and call
  one function would close the loop. Not blocking, but it's
  the only line of defense between "the dylib compiles" and
  "the dylib actually works."
- [N3] `bouncer-extension/README.md` documents the SQL surface
  but does not tell users **how** to load the dylib. Real users
  need either:
  - `sqlite3` CLI: `.load /path/to/libbouncer_ext` (with
    `load_extension` allowed); or
  - C API: `sqlite3_enable_load_extension(db, 1)` followed by
    `SELECT load_extension('/path/to/libbouncer_ext.dylib')`;
    or
  - rusqlite: `conn.load_extension_enable()` +
    `conn.load_extension(...)`.
  None of these are mentioned. This is the most common
  first-real-use foot-gun for SQLite extensions; worth a
  five-line README section before the next reader hits it.
- [N4] The plan's "Files likely to change" listed `SYSTEM.md`.
  The implementation correctly did not touch it (per IDD —
  baseline updates after acceptance). But the plan should
  arguably have predicted that — listing it among
  files-likely-to-change implies it will be touched in this
  phase. Cosmetic; a future plan could distinguish "files
  changed during this phase" from "files updated only after
  acceptance."

### Adversarial review

- [A1] **Nested-transaction trap.** Each SQL function calls
  `claim`/`renew`/`release` which internally start
  `BEGIN IMMEDIATE`. When the outer SQL is `SELECT
  bouncer_claim(...)` in autocommit mode, this works. When the
  outer SQL is part of an explicit transaction
  (`BEGIN; SELECT bouncer_claim(...); INSERT INTO log ...;
  COMMIT;`) the inner `BEGIN IMMEDIATE` will fail with
  "cannot start a transaction within a transaction." The
  spec-diff doesn't promise this works, but users will
  reasonably try it — wrapping coordination + audit logging in
  one explicit transaction is a normal pattern. Either:
  - document that `bouncer_*` SQL functions must run in
    autocommit mode (likely the right answer, lowest-cost), or
  - detect "already in a transaction" at the function entry
    and skip the `BEGIN IMMEDIATE` (using a SAVEPOINT
    instead), which is more code but matches user
    expectations.
  Worth pinning before this phase closes — it's the most
  likely source of confused bug reports.
- [A2] **Reentrancy via `ctx.get_connection()`.** Each scalar
  function calls `unsafe { ctx.get_connection() }` and then
  starts its own transaction on the same Connection that's in
  the middle of executing the outer `SELECT`. SQLite tolerates
  this in autocommit mode because the outer `SELECT` only
  holds a SHARED lock that the same connection can promote to
  RESERVED via `BEGIN IMMEDIATE` without deadlocking against
  itself. But this is genuinely subtle behavior that depends
  on SQLite's intra-connection locking model, and a future
  change to either rusqlite or SQLite that tightens this rule
  would break Phase 003. Worth a one-line comment near
  `attach_bouncer_functions` explaining the assumption — or a
  `#[test]` that pins it.
- [A3] **The SQL surface is now a public contract.** Once a
  caller wires `SELECT bouncer_claim(...)` into an app, the
  argument order, the return shape, and the function names
  are load-bearing. The spec-diff and plan committed to the
  current set; the implementation matches; future phases need
  to treat any change as a breaking API change to a
  cross-language boundary. Not a finding — just worth being
  honest that Phase 003 has narrowed the family's design
  space more than Phase 002 did.
- [A4] **Test file split, again.** Phase 002's implementation
  review (Round 003 N1) flagged that the wrapper tests had
  grown into one large file. Phase 003 added another ~230
  lines of SQL interop tests to the same file. The wrapper
  test module is now ~545 lines covering bootstrap,
  wrapper-only lease cycles, wrapper/core interop, AND
  SQL/Rust interop. A reader looking for "what does Phase 003
  prove?" has to grep. The user has previously chosen to keep
  tests in one file; this finding is purely a marker that the
  pattern continues, not an ask to revisit the choice.

### Review verdict

- Accepted with two real items to fix and one trap to
  document before this phase closes and `SYSTEM.md` is
  updated.

The implementation is clean: layering matches the
"core + extension + thin bindings" shape, the extension itself
is a 13-line FFI shim, lease semantics are not duplicated, and
every spec-diff verification item has a corresponding test.
14 + 17 tests pass; the dylib builds; the user's manual probe
loaded it.

To close the phase tightly:

1. **[N1]** Apply the clippy fix (`io::Error::other`). One line.
2. **[A1]** Decide and document the nested-transaction story:
   either "`bouncer_*` requires autocommit mode" in the
   `bouncer-extension/README.md`, or detect-and-savepoint in
   `attach_bouncer_functions`. Either is fine; punting it is
   not, because it's the most likely first-use failure.
3. **[N3]** Add five lines to `bouncer-extension/README.md`
   showing how to load the dylib (rusqlite, sqlite3 CLI,
   raw C). Without this, "extension builds and loads" is a
   contract claim that depends on the user already knowing
   SQLite extension semantics.

Nice-to-have for follow-up:

- **[N2]** an automated dylib-load test (rusqlite
  `load_extension` against the built artifact) so a future
  rename of the entrypoint cannot silently regress.
- **[A2]** a one-line comment near `attach_bouncer_functions`
  explaining the `ctx.get_connection()` + `BEGIN IMMEDIATE`
  reentrancy assumption.

Phase 003 is shippable as-is; the three "before close" items
above are about closing the gap between "works on the happy
path" and "fails informatively on the obvious wrong path."

## Decision Round 002

### Responding to

- Review Round 001 findings `[N1]`, `[N3]`, and `[A1]`

### Decisions

- [D6] Accept `[N1]` and apply the clippy fix in `to_sql_err`.
  Target:
  - `bouncer-honker/src/lib.rs`

- [D7] Accept `[A1]` by making the SQL mutator contract explicit:
  `bouncer_claim`, `bouncer_renew`, and `bouncer_release` are
  autocommit-mode helpers. Document that contract and pin the current
  failure mode with a test.
  Target:
  - `bouncer-honker/src/lib.rs`
  - `bouncer-extension/README.md`
  - `SYSTEM.md`

- [D8] Accept `[N3]` and add actual extension-loading instructions to
  `bouncer-extension/README.md`.
  Target:
  - `bouncer-extension/README.md`

- [D9] Defer `[N2]`. An automated dylib-load integration test is still a
  good idea, but it is not required to close Phase 003 because the
  extension build plus the one-off bundled-SQLite probe already proved
  the path once and the core SQL contract is pinned by automated tests.
  Target:
  - future phase or follow-up

- [D10] Defer `[A2]`. The code now documents the autocommit/nested
  transaction assumption near `attach_bouncer_functions`; no extra test
  beyond the pinned nested-transaction failure is required for Phase 003.
  Target:
  - none now

- [D11] Treat `[N4]` as closed by acceptance. `SYSTEM.md` was correctly
  left untouched during implementation and is updated only now, after
  review and response.
  Target:
  - `SYSTEM.md`

### Verification

- `cargo test`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo build -p bouncer-extension`

### Verdict

Phase 003 is accepted as the new baseline after these response changes.
