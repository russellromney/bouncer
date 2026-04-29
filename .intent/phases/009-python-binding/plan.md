# Plan

## Goal

Build the first Python binding for Bouncer.

The point is not to maximize Python surface area. The point is to prove
the Honker-family binding hypothesis against a second language:

- one SQLite file
- Bouncer-owned schema and semantics
- thin language wrapper
- no Redis-shaped sidecar
- no duplicate state machine

## Phase outcome

Coming out of Phase 007, Bouncer already has the Rust core, SQL
surface, Rust wrapper, wrapper transaction handle, and savepoint
hardening proof, and Phase 008 core-crate rename. At the end of
Phase 009, Bouncer should add:

- a Python package that can be installed locally for development
- a binding-owned database handle with explicit `bootstrap()`
- Python methods for `inspect`, `claim`, `renew`, and `release`
- a Python transaction/context-manager path for business writes plus
  lease mutations in one atomic boundary
- Python tests covering the binding contract and cross-language/file
  interop
- README / development docs for building and testing the Python package

At the end of Phase 009, Bouncer should not have:

- nested savepoints
- async Python APIs
- framework integrations
- caller-owned `sqlite3.Connection` integration
- a broader public API than the current Rust wrapper can justify

## Shape decision

Follow the Honker-family package convention visible in Knocker, plus a
standard PyO3/maturin package layout. This is not copying an existing
Knocker Python binding; it is using the same repo organization idea.

- create `packages/bouncer-py/`
- publish/import as Python package `bouncer`
- build native module `bouncer._bouncer_native` with PyO3 + maturin
- keep Python source under `packages/bouncer-py/python/bouncer/`
- keep the PyO3 crate name separate from the Rust wrapper crate, likely
  `bouncer-py`

This avoids renaming or moving the existing Rust wrapper at
`packages/bouncer`.

Do not add `packages/bouncer-py` to the root Cargo workspace in this
phase. Give the PyO3 package its own `[workspace]`, like Knocker's
Python package does, and build it through maturin. Root Rust checks
remain `cargo test -p bouncer -p bouncer-core`; Python checks own the
native extension build.

The native binding should call `bouncer-core` directly rather than
wrapping `packages/bouncer`. Rationale: Python cannot use Rust's
borrow-checked `Transaction<'db>` shape directly, so the binding needs
a runtime transaction guard anyway. Reuse the core state machine; keep
binding-edge policy tiny and explicit.

## Proposed Python API

The first user-facing surface should look roughly like:

```python
import bouncer

db = bouncer.open("app.sqlite3")
db.bootstrap()

result = db.claim("scheduler", "worker-a", ttl_ms=30_000)
if result.acquired:
    print(result.lease.token)
else:
    print(result.current.owner)
```

Transactional work should look roughly like:

```python
with db.transaction() as tx:
    tx.execute("INSERT INTO jobs(payload) VALUES (?)", ["work"])
    claim = tx.claim("scheduler", "worker-a", ttl_ms=30_000)
    if not claim.acquired:
        tx.rollback()
```

Notes:

- Use `ttl_ms` in Phase 009. It is explicit, easy to test, and matches
  the SQLite surface. Friendlier `datetime.timedelta` or seconds
  coercion can come later if Python callers want it.
- Return Python result objects/dataclasses:
  - `LeaseInfo`
  - `ClaimResult`
  - `RenewResult`
  - `ReleaseResult`
- Expose minimal SQL execution on the transaction handle because
  otherwise Python cannot prove "business write + lease mutation" in
  one boundary.
- `tx.execute(sql, params=None)` binds positional parameters and
  returns the affected row count. No query surface is required for V1
  unless tests prove it is needed.
- Map native failures to one umbrella `bouncer.BouncerError` in V1,
  with clear messages from SQLite/core errors. More specific exception
  subclasses can be added later without changing successful result
  shapes.
- Keep savepoints out. The Rust wrapper has one savepoint level, but
  Python should first prove the primary transaction story.

## Implementation steps

1. Add Python workspace tooling.
   - root `pyproject.toml` for dev dependencies
   - `Makefile` with pinned Rust, extension-build, Python-build, and
     Python-test commands
   - ignore local virtualenv/build artifacts if needed

2. Add `packages/bouncer-py`.
   - `Cargo.toml` with `cdylib`, PyO3, rusqlite, and local
     `bouncer-core` dependency
   - a local `[workspace]` section so the PyO3 cdylib package does not
     participate in root workspace feature unification
   - `pyproject.toml` using maturin
   - `python/bouncer/__init__.py`
   - small Python model/result wrappers if native returns plain shapes

3. Implement the native binding.
   - open a rusqlite connection to a database path
   - keep bootstrap explicit
   - call `bouncer-core` helpers for all lease semantics
   - use system time only at the binding edge for convenience methods
   - use a runtime transaction guard for the binding-owned connection
     so overlapping Python transactions fail loudly
   - implement transaction `execute(...)` and the lease mutators inside
     the active transaction
   - bind SQL parameters positionally with rusqlite rather than string
     interpolation

4. Add tests.
   - explicit bootstrap is required
   - full claim / busy / inspect / renew / release cycle
   - Python claim visible to the SQL extension on the same file
   - SQL-created lease visible to Python
   - transaction commit persists business write and lease mutation
   - transaction rollback discards business write and lease mutation
   - overlapping transactions fail loudly
   - errors map to `bouncer.BouncerError`

   The cross-surface tests should build `bouncer-extension`, load it
   through stdlib `sqlite3.enable_load_extension(True)`, and call
   `bouncer_owner` / `bouncer_token` against the same database file.

5. Update docs after implementation.
   - root README "What exists today"
   - package README or Python usage section
   - ROADMAP current status / next steps
   - CHANGELOG Phase 009
   - SYSTEM.md only after tests prove the binding baseline

## Files likely to change

- `.intent/phases/009-python-binding/*`
- `Cargo.toml`
- `.gitignore`
- `pyproject.toml`
- `Makefile`
- `README.md`
- `ROADMAP.md`
- `CHANGELOG.md`
- `SYSTEM.md`
- `packages/bouncer-py/**`
- `tests/**`

## Areas that should not be touched

- `bouncer-core` lease semantics
- existing Rust wrapper public behavior
- SQL function names
- nested savepoint surface

## Risks and assumptions

- The biggest risk is overbuilding Python ergonomics before the binding
  proves the core contract. Prefer explicit `ttl_ms` and small result
  objects.
- A binding-owned rusqlite connection cannot participate in a caller's
  existing Python `sqlite3.Connection` transaction. That is acceptable
  for Phase 009; the SQL extension is the better future path for
  caller-owned Python connections.
- PyO3 lifetime management should avoid exposing borrowed Rust
  transaction handles directly. Use a binding-owned connection and a
  runtime transaction guard if needed.
- Python tests should not duplicate every core edge case. They should
  prove thin delegation, interop, and transaction behavior.

## Pinned commands

The implementation should make these commands work:

```bash
make test-rust
make build-ext
make build-py
make test-python
make test
```

Expected command meanings:

- `make test-rust`
  runs `cargo test -p bouncer -p bouncer-core`
- `make build-ext`
  builds `bouncer-extension` for Python SQL-surface interop tests
- `make build-py`
  runs `uv run --group dev maturin develop --manifest-path packages/bouncer-py/Cargo.toml`
- `make test-python`
  runs the Python tests after `build-py` and `build-ext`
- `make test`
  runs Rust plus Python checks
