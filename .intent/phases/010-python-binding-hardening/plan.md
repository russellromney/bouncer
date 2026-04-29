# Plan

## Goal

Harden the Python binding's public API contract before any further
bindings or Honker integration work. Phase 009 proved the binding
hypothesis and shipped a working Python surface; review surfaced
small API-honesty gaps that should close before V1 callers depend
on the current shape.

## Phase outcome

At the end of Phase 010, Bouncer should have:

- a Python `Transaction` that is honestly context-manager-first:
  `BEGIN IMMEDIATE` opens inside `__enter__`, not inside
  `Bouncer.transaction()`
- a clear failure mode for the previously-undocumented
  `tx = db.transaction(); tx.claim(...)` path
- one less safety-net path: `Transaction.__del__` removed if the
  context-manager-first change leaves it without a real role
- `packages/bouncer-py/Cargo.toml` on the same Rust edition as the
  rest of the family
- a Python README section pointing `sqlite3.Connection`-owning
  callers at the SQL extension path
- a root README example that shows the three caller surfaces side
  by side so first-time readers can see the boundary at a glance

At the end of Phase 010, Bouncer should not have:

- any new Python verbs or result types
- any new SQL surface
- async Python APIs
- nested savepoints (still future)
- another non-Rust binding
- the deterministic-simulation harness

## Mapping from spec diff to implementation

The spec diff says we are tightening an existing surface, not adding
one. So the implementation plan should produce:

1. a context-manager-first `Transaction` whose pre-`__enter__` state
   is observable, fail-fast, and tested
2. a deliberate decision on `Transaction.__del__` (remove or keep
   with a documented reason)
3. one Cargo edition update
4. two small README updates

## Phase decisions already made

- This phase is hardening, not new product surface.
- The primitive contract from Phase 007 stays frozen.
- `bouncer-core` remains the binding link target. We do not start
  wrapping `packages/bouncer` or routing the binding through the
  SQL extension.
- The fix for `tx = db.transaction(); tx.claim(...)` is to make it
  a clean error, not to make it work. Honesty over convenience.

## Proposed approach

### 1. Context-manager-first Transaction

Move the eager `BEGIN IMMEDIATE` out of `Bouncer.transaction()` and
into `Transaction.__enter__`.

- `Bouncer.transaction()` returns a fresh `Transaction(self._native)`
  with no native side effect.
- `Transaction.__enter__` calls `self._native.begin_transaction()`
  the first time, sets `_entered = True`, and returns `self`.
- Re-entry is an error. `__enter__` raises `BouncerError` if
  `_entered` is already true or `_finished` is already true.
- A new internal `_ensure_active()` replaces today's `_ensure_open`
  pattern: must be entered, must not be finished. Every `tx.*`
  verb (`execute`, `inspect`, `claim`, `renew`, `release`,
  `commit`, `rollback`) calls it.

### 2. Decision on `Transaction.__del__`

After step 1, a `Transaction` that was never entered holds no
native transaction state. The native `Drop for NativeBouncer`
already rolls back any orphaned `transaction_active = True` if
the underlying handle is torn down. The Python `__del__`
fallback covers no remaining real failure mode.

The default decision is to remove it. If implementation surfaces
a real safety role, document the role in `_bouncer.py` and keep
the method. Do not keep it as belt-and-suspenders.

### 3. Rust edition alignment

`packages/bouncer-py/Cargo.toml` currently uses `edition = "2024"`.
`bouncer-core`, `bouncer-extension`, and `packages/bouncer` use
`2021`. Move `bouncer-py` to `2021` to match its three siblings
inside Bouncer. Survey of the broader Honker family
(`grep "^edition"`) shows Honker and Knocker on `2024`, so Bouncer
is locally consistent on `2021` and the family is split. Bouncer-
wide alignment up to `2024` is out of scope for this phase. The
change here is a one-line edit; PyO3 0.28 supports both editions.

### 4. Python README addition

Add a short section to `packages/bouncer-py/README.md`:

> If you already own a `sqlite3.Connection` and want Bouncer
> semantics on it, load the `bouncer-extension` SQLite loadable
> extension instead. The Python binding owns its own SQLite
> connection and does not participate in a connection your code
> already manages.

Include a one-paragraph example or a pointer to the existing
extension load pattern in the cross-surface tests.

### 5. Root README example

Add one short block to `README.md` under "What exists today" or in
a new "Choosing a surface" section showing the three caller
surfaces side by side:

- SQL extension for SQL-only callers (load
  `libbouncer_ext.{dylib,so}` into any SQLite client)
- Python binding for typed Python callers (`bouncer.open(path)`)
- Rust wrapper for Rust callers (`packages/bouncer`)

Keep it tight: one snippet per surface, no full lease cycle.

## Build order

1. Move `BEGIN IMMEDIATE` from `Bouncer.transaction()` into
   `Transaction.__enter__`. Keep `Bouncer.transaction()` as a
   trivial constructor.
2. Add `_entered` and `_ensure_active()` to `Transaction`. Update
   every verb to use the active guard. Pin single-use semantics in
   `__enter__`. Pin `_entered = False` on `begin_transaction`
   failure so the instance can be re-entered.
3. Remove `Transaction.__del__`.
4. Drop `Transaction` from `bouncer.__all__` and from the public
   re-export in `__init__.py`.
5. Add tests:
   - `test_transaction_without_enter_raises` —
     `tx = db.transaction()`, then `tx.claim(...)` raises
     `BouncerError`.
   - `test_transaction_without_enter_does_not_lock_database` —
     direct test using a second stdlib `sqlite3.Connection` to
     the same file with a short `busy_timeout`; the second
     connection writes successfully, proving the unentered
     `Transaction` did not hold a write lock.
   - `test_transaction_is_single_use` — `with tx:` after a clean
     exit raises `BouncerError`.
   - `test_transaction_inspect_returns_live_lease` — direct
     happy-path test for `tx.inspect`.
   - `test_transaction_renew_extends_lease` — direct happy-path
     and rejection-path test for `tx.renew`.
   - `test_transaction_release_clears_owner` — direct
     happy-path test for `tx.release`.
   - `test_bouncer_error_covers_non_lease_errors` — a SQL syntax
     error in `tx.execute` raises `BouncerError`.
   - `test_tx_execute_runs_only_first_statement` — regression
     test for the rusqlite single-statement silent-drop behavior.
6. Update `packages/bouncer-py/Cargo.toml` edition to `2021`.
7. Update `packages/bouncer-py/README.md`:
   - add a "Python `sqlite3.Connection` users" section with a
     five-line working snippet showing
     `sqlite3.connect → enable_load_extension → load_extension →
     SELECT bouncer_bootstrap()`
   - add a one-line note that `tx.execute` is single-statement
8. Update root `README.md` with the three-surface example block:
   SQL extension, Python binding, Rust wrapper. One snippet per
   surface, no full lease cycle.
9. Run `make test`. Confirm Phase 009's 11 Python tests still pass
   plus the new tests.
10. Update `ROADMAP.md` (next-steps), `CHANGELOG.md` (Phase 010
    entry), and `SYSTEM.md` (binding-baseline updates) at
    closeout.

## Files likely to change

- `.intent/phases/010-python-binding-hardening/*`
- `packages/bouncer-py/python/bouncer/_bouncer.py`
- `packages/bouncer-py/Cargo.toml`
- `packages/bouncer-py/README.md`
- `packages/bouncer-py/tests/test_bouncer.py`
- `README.md`
- `ROADMAP.md`
- `CHANGELOG.md`
- `SYSTEM.md`

## Areas that should not be touched

- `bouncer-core` lease semantics
- `bouncer-extension` surface or build
- `packages/bouncer` Rust wrapper public behavior
- result-type shapes on either side of the binding
- pinned `make` targets, `uv` workflow, or `[workspace]` isolation

## Risks and assumptions

- The biggest risk is scope drift: hardening phases tend to attract
  unrelated cleanup. Keep this phase to the five named items.
- The existing 11 Python tests all use
  `with db.transaction() as tx:`, so the move into `__enter__`
  should not break any test. If a test does break, treat that as
  evidence the test was relying on a path the new contract
  forbids, and fix the test rather than the contract.
- The native `transaction_active` flag still owns the runtime
  exclusivity guarantee. Moving the begin into `__enter__` does not
  change the guard's behavior; only when it fires.
- Removing `__del__` is a documented behavior change, but no public
  test or example relies on it.

## Out of scope but tracked

These are real but live elsewhere:

- Three missing in-transaction Python verb tests (`tx.inspect`,
  `tx.renew`, `tx.release`) — Phase 009 Review Round 003 `[F1]`
  and Round 004 `[F2]`. Small, mechanical; could roll into this
  phase if the user wants, but not part of the named scope.
- `bouncer-extension` first-class Rust integration test — Phase 009
  Review Round 004 `[F1]`. A separate testing-coverage phase fits
  better than folding it into this hardening pass.
- `BouncerError` non-lease error test — Round 004 `[F3]`.
- `tx.execute` single-statement contract pin — Round 004 `[F4]`.
- Cross-binding parity test — Round 004 `[F5]`.
