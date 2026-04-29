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

## Review Round 002

### Reviewing

- the post-Decision-Round-002 state of `plan.md` and `spec-diff.md`,
  now using `bouncer-core` and Phase 009 numbering
- the landed Phase 008 core-crate rename (commits `5aaf544 Rename
  Bouncer core crate` and `4a5c35f Record Phase 008 commit trace`;
  `bouncer-core/` exists, `bouncer-honker/` is gone)
- whether Round 1 findings `[F1]` through `[F9]` are pinned in
  writing in the right place
- whether the now-tighter plan surfaces any new questions that should
  also be pinned before implementation

### Round 1 acceptances pin-check

- `[F1]` wording — pinned in `plan.md` "Shape decision" section
- `[F2]` architecture — pinned in `plan.md` "Shape decision" with
  rationale and in `spec-diff.md` "What changes"
- `[F3]` cross-surface verification — pinned in `plan.md` test list
  and `spec-diff.md` "How we will verify it"
- `[F4]` cdylib + workspace isolation — pinned in `plan.md`
  "Shape decision" and step 2
- `[F5]` umbrella `bouncer.BouncerError` — pinned in `plan.md`
  Notes
- `[F6]` `tx.execute` returns affected row count — pinned in
  `plan.md` Notes
- `[F7]` positional parameter binding contract — pinned in
  `plan.md` step 3 and `spec-diff.md` "What changes"
- `[F8]` no PyPI publication in this phase — pinned in
  `spec-diff.md` "What does not change"
- `[F9]` pinned make targets — `plan.md` "Pinned commands" lists
  five concrete invocations

All nine are now in the right place, and `bouncer-core` is used
consistently in the active 009 text.

### Findings

- [F10] **Context-manager exit state machine is unspecified.** The
  transactional example shows
  `with db.transaction() as tx: ... tx.rollback()`. After explicit
  rollback, `__exit__` will still run on block exit. The plan should
  specify the state machine: `__exit__` on normal block exit commits
  if the tx is not already finished; on exception rolls back;
  explicit `tx.commit()` or `tx.rollback()` marks the tx done and
  `__exit__` becomes a no-op. This is the kind of state machine that
  is painful to retrofit if V1 callers wrote against ambiguous
  behavior.

- [F11] **Autocommit verbs during an open transaction are
  unspecified.** Rust prevents this at compile time via `&mut self`
  on `Bouncer::transaction()`. Python's runtime guard catches
  `db.transaction()` while one is open. But what about `db.claim(...)`
  on the same `Bouncer` while a `with db.transaction() as tx:` is
  active? Two reasonable answers: (a) runtime error with a clear
  message pointing at `tx.claim`; or (b) silently delegate to the
  active transaction the way `BouncerRef` does in autocommit-mode
  dispatch. (a) parallels the Rust compile-time guarantee and is
  the safer V1 default. Pick one.

- [F12] **PyO3 `Send`/`Sync` posture is implicit.** A `#[pyclass]`
  holding a `rusqlite::Connection` must handle that `Connection` is
  `Send` but not `Sync`. Two paths: `Mutex<Connection>` becomes
  `Send + Sync` and lets the binding optionally release the GIL on
  long calls; or `pyclass(unsendable)` keeps the object on its
  creating thread and is simpler to reason about. For V1 the
  simplest correct choice is "hold the GIL during all native calls
  plus `pyclass(unsendable)`," which serializes Python threading
  through the binding and matches the binding-owned-connection
  mental model. Pin which way the binding goes so the implementer
  does not invent a third path.

- [F13] **Result-type shape is not pinned.** The example shape is
  flat (`result.acquired`, `result.lease`, `result.current`) rather
  than enum-with-data (`ClaimResult::Acquired(LeaseInfo)` vs
  `Busy(LeaseInfo)`). The flat model is friendlier in Python but
  loses the discriminated-union discipline. Pin which model V1
  uses. Also pin whether `LeaseInfo` and friends are pure-Python
  `@dataclass` types layered over plain native shapes, or PyO3
  `#[pyclass]` types returned directly. Pure-Python dataclasses on
  top of plain native dicts is the easier-to-evolve choice.

- [F14] **`uv` is now a hard dependency for the dev workflow.**
  `make build-py` runs `uv run --group dev maturin develop`. That
  is a fine choice but worth stating explicitly: developers need
  `uv` installed; non-`uv` paths are not supported in V1. Other
  family repos use different Python toolchains, so making this
  decision visible avoids drift.

- [F15] **`bouncer-extension` build artifact path is unspecified.**
  Cross-surface tests load `bouncer-extension` via
  `sqlite3.enable_load_extension`. The artifact lives at
  `target/{debug,release}/libbouncer_extension.{dylib,so}` and the
  exact path varies by OS and build profile. Specify: `make build-ext`
  builds in a fixed profile and the Python tests resolve the artifact
  from a known location, or fail loudly with a clear message if the
  artifact is missing.

- [F16] **Test directory location.** `plan.md` "Files likely to
  change" lists `tests/**` but the binding tests should live under
  `packages/bouncer-py/tests/` to follow the package boundary.
  Tighten to the package-local path so the root `tests/` tree does
  not accumulate cross-cutting concerns.

### Things checked and fine

- Phase 008 rename is real, complete, and consistent in 009 active
  text. Historical `bouncer-honker` mentions are correctly preserved
  in append-only review text per `[D15]`.
- The three-layer name story is internally consistent: `bouncer-py`
  (cargo crate, never user-visible), `bouncer` (Python package
  name), `bouncer._bouncer_native` (native module).
- Pinned `make test-rust`, `build-ext`, `build-py`, `test-python`,
  `test` cover the F9 ask.
- "No `BouncerRef` analogue, no nested savepoints, no caller-owned
  `sqlite3.Connection`" exclusions still hold.
- Runtime tx guard plus binding-owned connection is the right
  architecture for PyO3.
- The cross-surface verification path through stdlib `sqlite3`
  loading `bouncer-extension` is the right shape and is now
  explicit.

### Verdict

The plan is meaningfully sharper than the Phase 008 version Round 1
reviewed. `[F10]` through `[F16]` are all "pin the implementation
detail in writing before code lands" — the same kind of small
contract decisions that `[F2]` through `[F9]` were last round. None
blocks implementation; collectively they prevent V1 caller code
from being written against ambiguous behavior.

Recommended next step: a Decision Round 004 that resolves `[F10]`
through `[F16]` with one-sentence answers, then implementation can
proceed.

## Decision Round 004

### Responding to

- Review Round 002 `[F10]` through `[F16]`
- cross-session agreement that these are API-contract pins, not
  Python-rethink findings, and should be answered before code lands
- correction of an artifact-name error in `[F15]`: the
  `bouncer-extension` cdylib `[lib] name` is `bouncer_ext`, so the
  built artifact is `libbouncer_ext.{dylib,so}` /
  `bouncer_ext.dll`, not `libbouncer_extension`

### Decisions

- [D16] Accept `[F10]` with explicit state machine.
  `with db.transaction() as tx:` commits on clean block exit, rolls
  back on exception, and explicit `tx.commit()` or `tx.rollback()`
  marks the transaction finished so `__exit__` becomes a no-op. Any
  operation on a finished transaction raises `BouncerError`.
  Target:
  - `spec-diff.md`
  - `plan.md`

- [D17] Accept `[F11]` with sharpening. While a transaction is
  active, all top-level `db.*` lease operations
  (`claim`, `renew`, `release`, `inspect`) raise `BouncerError`.
  Callers must use the `tx` object until it finishes. This keeps the
  mental model "once you enter `with db.transaction() as tx`, use
  `tx`" without case analysis on read vs write verbs.
  Target:
  - `spec-diff.md`
  - `plan.md`

- [D18] Accept `[F12]`. The native `#[pyclass]` types use
  `pyclass(unsendable)`, hold the GIL during all native calls, and
  do not support cross-thread sharing in V1. Multiple Python
  threads that need their own handle should call `bouncer.open(...)`
  separately.
  Target:
  - `plan.md`

- [D19] Accept `[F13]` with the flat shape and pure-Python wrappers.
  The native layer returns plain shapes (dicts or simple tuples),
  and Python exposes pure `@dataclass` result objects:
  - `LeaseInfo` carries `name`, `owner`, `token`,
    `lease_expires_at_ms`
  - `ClaimResult` carries `acquired: bool` plus `lease` and
    `current`
  - `RenewResult` carries `renewed: bool` plus `lease` and
    `current`
  - `ReleaseResult` carries `released: bool` plus `name`, `token`,
    and `current`

  Flat shape is more idiomatic in Python and easier to evolve
  without breaking native ABI.
  Target:
  - `spec-diff.md`
  - `plan.md`

- [D20] Accept `[F14]`. `uv` is a hard development dependency for
  Phase 009. There is no documented non-`uv` path in V1.
  Target:
  - `plan.md`

- [D21] Accept `[F15]` with the artifact-name correction.
  `make build-ext` builds `bouncer-extension` in the `debug`
  profile by default. The Python tests resolve the artifact at
  `target/debug/libbouncer_ext.{dylib,so}` on macOS and Linux and
  `target/debug/bouncer_ext.dll` on Windows. Tests must fail
  loudly with a clear message if the artifact is missing.
  Target:
  - `plan.md`

- [D22] Accept `[F16]`. Python tests live under
  `packages/bouncer-py/tests/`, not the root `tests/` tree.
  Target:
  - `plan.md`

### Verdict

Phase 009 implementation is ready once `spec-diff.md` and
`plan.md` are patched to reflect `[D16]` through `[D22]`. After
those pins land, code can proceed without further plan rounds.
