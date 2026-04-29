# Reviews And Decisions

This file is append-only.

- Review rounds are written in Session B.
- Decision rounds are written in Session A in response to findings.
- Do not rewrite earlier review text to make it look resolved.

## Decision Round 001

### Responding to

- completion of Phase 007 core hardening
- human decision that nested savepoints should be tracked as a future
  proposal rather than built next
- roadmap direction that the next real proof should be one non-Rust
  binding

### Decisions

- [D1] Phase 008 is the first Python binding.
  Target:
  - `spec-diff.md`
  - `plan.md`

- [D2] The first Python binding should prove the binding-owned path:
  open by database path, explicit bootstrap, lease verbs, and a
  sanctioned transaction/context-manager path.
  Target:
  - `spec-diff.md`
  - `plan.md`

- [D3] Do not include nested savepoints in Phase 008. They are now a
  roadmap future proposal and should come back only when a caller story
  needs them.
  Target:
  - `ROADMAP.md`
  - `spec-diff.md`
  - `plan.md`

- [D4] Do not promise caller-owned Python `sqlite3.Connection`
  integration in Phase 008. The SQL extension is the likely future path
  for callers that already own a Python SQLite transaction.
  Target:
  - `spec-diff.md`
  - `plan.md`

### Verdict

Phase 008 is ready for Session B review before implementation.

## Review Round 001

### Reviewing

- the Phase 008 `plan.md`, `spec-diff.md`, and Decision Round 001
- the post-Phase-007 state of the core (`bouncer-honker`),
  SQL extension (`bouncer-extension`), and Rust wrapper
  (`packages/bouncer`)
- the central pre-implementation question: **is core Bouncer V1
  complete enough to freeze while we prove bindings?**

### Central question: is the core ready to freeze?

Yes, with no blocking caveats.

The primitive surface that the Python binding will exercise is
already settled and proven through seven phases:

- one named resource → at most one live owner
- monotonic fencing token across expiry, release, and re-claim
- four verbs: `inspect`, `claim`, `renew`, `release`
- two transaction shapes: autocommit and sanctioned
  `BEGIN IMMEDIATE`
- one nested boundary: sanctioned `Savepoint` with terminal
  `commit` / `rollback`
- explicit time on every core helper (`now_ms`)
- explicit bootstrap; no implicit schema creation
- pragma-neutral wrapper; caller owns connection policy

Result types (`LeaseInfo`, `ClaimResult`, `RenewResult`,
`ReleaseResult`) are stable and have shipped through both Rust and
SQL surfaces with cross-surface interop tests. They are the right
shapes to freeze and mirror in Python.

The roadmap's remaining ambitions (DST clock seam, batch APIs, a
listing surface, schema migrations) are all *additive* relative to
what the binding will exercise. None of them require breaking the
current contract; none of them block Phase 008. Implementing the
binding now will not paint us into a corner.

So the answer to the codex framing is: freeze the core, prove the
binding hypothesis, do not open another core implementation phase
unless the binding work surfaces a real gap.

### Plan strengths

- Phase outcome and non-goals are tight. Nested savepoints, async,
  framework integrations, caller-owned `sqlite3.Connection`, and
  broader API surface are all explicitly excluded.
- Defaults match the existing surface: explicit `ttl_ms`, explicit
  bootstrap, sanctioned transaction with `execute(...)` for business
  writes, small Python result objects.
- Test list covers the right categories: full lifecycle, both
  interop directions, transaction commit and rollback, overlapping
  transactions failing loudly.
- The recognition that PyO3 cannot represent a borrow-checked
  `Transaction<'db>` is correct; the plan's "binding-owned
  connection plus runtime transaction guard" is the right shape.

### Findings

- [F1] **The "Knocker package pattern" reference is slightly
  aspirational.** Knocker has `packages/knocker` and
  `packages/knocker-node` — there is no in-family Python binding to
  copy from. The plan's use of "Knocker pattern" should be read as
  "the `packages/<name>-<lang>` organizational pattern adapted for
  PyO3," not "follow an existing Python precedent." Worth either
  rewording the plan or naming a concrete external reference (for
  example, the maturin/PyO3 layout used by `pydantic-core` or
  `polars`).

- [F2] **Binding architecture decision is implicit and worth
  making explicit.** The plan says "call `bouncer-honker` helpers
  for all lease semantics," which means the Python binding skips
  the Rust wrapper (`packages/bouncer`) and reaches into the core
  directly. This is a defensible choice — it avoids the lifetime
  problem of exposing `Transaction<'db>` to Python — but it means
  duplicating the wrapper's "autocommit vs in-tx dispatch" and
  "tx exclusivity guard" logic in the Python binding. The
  alternative (wrap `packages/bouncer` and use a
  `Mutex<Connection>` to give PyO3 a `'static` view) is more code
  on the Rust side but reuses the wrapper's compile-time
  exclusivity. The plan should pin this trade-off in `spec-diff.md`
  with a one-sentence rationale rather than leaving it implied.

- [F3] **Cross-surface verification mechanism is unspecified.**
  The plan calls for "Python claim visible to Rust core or SQL
  surface" and "Rust or SQL-created lease visible to Python," but
  does not pick a verification mechanism. The cleanest is to load
  `bouncer-extension` into a stdlib `sqlite3.Connection` from the
  Python tests and call `bouncer_owner` / `bouncer_token` SQL
  functions on the same file. That keeps verification inside one
  test runner and exercises the SQL surface. Worth naming this in
  the plan rather than leaving it as a test-time decision.

- [F4] **PyO3 cdylib + Cargo workspace integration is unaddressed.**
  Bouncer is a Cargo workspace. PyO3 `cdylib` crates often
  interact awkwardly with `cargo test --workspace` because the
  `cdylib` target conflicts with library-mode dependents
  (`bouncer-extension` already triggered exactly this earlier; see
  Phase 006 notes about `cargo test --workspace` failing on the
  loadable-extension feature unification). The plan should pick
  one: include `packages/bouncer-py` in the workspace and accept
  the same per-crate test discipline, or exclude it and document
  the build invocation. Either is fine; silence is not.

- [F5] **Error type mapping is unspecified.** The Rust wrapper has
  `Error::Sqlite`, `Error::Core(core::Error::*)`, `Error::SystemTime`,
  `Error::SystemTimeTooLarge`, `Error::DurationTooLarge`. The
  Python binding needs a deliberate mapping: a single
  `bouncer.BouncerError` umbrella, multiple specific exceptions
  (`bouncer.InvalidTtlError`, `bouncer.SqliteError`, etc.), or a
  result-typed error variant on the call return. The plan should
  pick one shape before implementation; this is the kind of
  decision that locks in early and is painful to change after V1
  callers exist.

- [F6] **`tx.execute(...)` return shape is unspecified.** Plan
  says "Expose minimal SQL execution on the transaction handle."
  Reasonable interpretations: returns affected row count
  (DB-API style), returns nothing, returns rows. V1 should be the
  smallest thing that lets tests prove the "business write plus
  lease mutation" boundary, which is the affected-row count or
  unit. Worth specifying explicitly so reviewers and the
  implementer agree. If `query` is also needed for tests, name it
  separately from `execute`.

- [F7] **Parameter binding contract should be explicit.** The
  plan's example uses `tx.execute("INSERT INTO jobs(payload) VALUES (?)", ["work"])`.
  The implementation must bind these positionally as parameters,
  not interpolate. Standard PyO3 + rusqlite, but worth a sentence
  in `spec-diff.md` so it is non-negotiable rather than implied.

- [F8] **Distribution scope should be confirmed.** Plan says
  "installed locally for development" via `maturin develop`. PyPI
  publication is presumably out of scope for Phase 008.
  `spec-diff.md` should say so, since "distribute the package"
  vs "prove the binding shape locally" are very different scopes
  and "install locally" is ambiguous between them.

- [F9] **Test runner invocation should be pinned.** Plan says
  "documented commands for Rust + Python tests." That should be a
  concrete line: for example, `make test` invoking
  `cargo test -p bouncer -p bouncer-honker && maturin develop -m packages/bouncer-py/Cargo.toml && pytest packages/bouncer-py/tests`.
  Pinning this matters because Phase 006 already demonstrated that
  unspecified workspace test invocations can mask feature-
  unification regressions.

### Things checked and fine

- Result types (`LeaseInfo`, `ClaimResult`, `RenewResult`,
  `ReleaseResult`) are stable enough to mirror in Python.
- The "no `BouncerRef` analogue" choice is correct — Python cannot
  represent a borrow.
- The "no nested savepoints" choice matches Decision Round 001
  [D3] and tracks Phase 007's intentional gap.
- The "no caller-owned `sqlite3.Connection` integration" choice
  matches [D4] and is the right line to hold.
- Explicit `ttl_ms` in V1 is the right call; convenience time
  coercion is post-V1 ergonomics.

### Verdict

The core is ready to freeze and the plan is ready to implement,
once `[F2]` through `[F9]` are pinned in `plan.md` and / or
`spec-diff.md`. None of the findings is blocking on its own; they
are the kind of small decisions that should be locked in writing
before code lands so reviewers and implementers agree on the
contract. `[F1]` is a wording note, not a decision.

Recommended next step: a Decision Round 002 that resolves `[F2]`
through `[F9]` with one-sentence answers, then Phase 008
implementation can proceed.

## Decision Round 002

### Responding to

- Review Round 001 `[F1]` through `[F9]`
- human confirmation that core Bouncer V1 should freeze before Python
  implementation

### Decisions

- [D5] Accept `[F1]` as wording cleanup. The plan now says the Python
  layout follows the Honker-family package convention visible in
  Knocker plus a standard PyO3/maturin layout, not an existing Knocker
  Python precedent.
  Target:
  - `plan.md`

- [D6] Accept `[F2]`. The Python native binding will call
  `bouncer-honker` directly for lease semantics instead of wrapping
  `packages/bouncer`, because Python needs a runtime transaction guard
  regardless and cannot reuse Rust's borrow-checked transaction handle
  shape directly.
  Target:
  - `spec-diff.md`
  - `plan.md`

- [D7] Accept `[F3]`. Cross-surface Python tests should build
  `bouncer-extension`, load it through stdlib `sqlite3`, and verify
  `bouncer_owner` / `bouncer_token` on the same database file.
  Target:
  - `spec-diff.md`
  - `plan.md`

- [D8] Accept `[F4]`. `packages/bouncer-py` should not join the root
  Cargo workspace in this phase; it should use its own local
  `[workspace]` and be built through maturin.
  Target:
  - `plan.md`

- [D9] Accept `[F5]`. V1 maps native failures to one umbrella
  `bouncer.BouncerError` with clear messages; specific exception
  subclasses can be added later.
  Target:
  - `plan.md`

- [D10] Accept `[F6]`. `tx.execute(sql, params=None)` binds positional
  parameters and returns affected row count; no query surface is in V1
  unless tests prove it is needed.
  Target:
  - `plan.md`

- [D11] Accept `[F7]`. Parameter binding is a contract: values go
  through rusqlite positional binding, never string interpolation.
  Target:
  - `spec-diff.md`
  - `plan.md`

- [D12] Accept `[F8]`. Phase 008 is local development install and test
  only; PyPI packaging/release is out of scope.
  Target:
  - `spec-diff.md`

- [D13] Accept `[F9]`. The implementation must add pinned make targets
  for Rust checks, extension build, Python build, Python tests, and the
  combined test path.
  Target:
  - `plan.md`

### Verdict

Phase 008 implementation can proceed after these plan pins. Core V1 is
frozen unless Python work reveals a concrete contradiction in the
settled contract.

## Decision Round 003

### Responding to

- human observation that `bouncer-honker` implies the wrong dependency
  direction if Honker eventually depends on Bouncer
- Phase 008 core-crate rename from `bouncer-honker` to `bouncer-core`

### Decisions

- [D14] The Python binding phase is renumbered from Phase 008 to
  Phase 009 so the crate rename can land first.
  Target:
  - folder path
  - `spec-diff.md`
  - `plan.md`

- [D15] Earlier review text in this file still refers to Phase 008 and
  `bouncer-honker` because it was written before the rename. Do not
  rewrite it; the phase's active spec and plan now use Phase 009 and
  `bouncer-core`.
  Target:
  - this file

### Verdict

Python remains the next product implementation phase after the
`bouncer-core` rename lands.
