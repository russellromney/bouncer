# Reviews And Decisions

This file is append-only.

- Review rounds are written in Session B.
- Decision rounds are written in Session A in response to findings.
- Do not rewrite earlier review text to make it look resolved.

## Decision Round 001

### Responding to

- completion of Phase 009 (the first Python binding) and the
  resulting Review Rounds 003 and 004
- direct human framing: hardening / API honesty pass before any
  Honker integration or further bindings
- the specific gaps named: context-manager-first transactions,
  `Transaction.__del__` cleanup, Rust edition alignment, docs for
  callers who already own a `sqlite3.Connection`, and a tiny
  three-surface README example

### Decisions

- [D1] Phase 010 is a Python-binding hardening pass. No new
  product surface, no new bindings, no Honker integration.
  Target:
  - `spec-diff.md`
  - `plan.md`
  - `ROADMAP.md`

- [D2] `Bouncer.transaction()` no longer eagerly opens
  `BEGIN IMMEDIATE`. `Transaction` is honestly context-manager-first;
  `__enter__` opens the transaction and `__exit__` / explicit
  `commit` / `rollback` finishes it. Calling any `tx.*` verb before
  `__enter__` raises `bouncer.BouncerError`.
  Target:
  - `spec-diff.md`
  - `plan.md`
  - `packages/bouncer-py/python/bouncer/_bouncer.py`
  - `packages/bouncer-py/tests/test_bouncer.py`

- [D3] `Transaction.__del__` is removed by default. After the
  context-manager-first change, the Python object holds no native
  transaction state until `__enter__` runs, and the native
  `Drop for NativeBouncer` already covers handle teardown. If
  implementation surfaces a real remaining safety role, document
  the role in `_bouncer.py` and keep the method; do not keep it as
  belt-and-suspenders.
  Target:
  - `spec-diff.md`
  - `plan.md`
  - `packages/bouncer-py/python/bouncer/_bouncer.py`

- [D4] `packages/bouncer-py/Cargo.toml` aligns to Rust edition
  `2021`, matching `bouncer-core` and `bouncer-extension`.
  Target:
  - `plan.md`
  - `packages/bouncer-py/Cargo.toml`

- [D5] `packages/bouncer-py/README.md` adds a short section telling
  callers who already own a `sqlite3.Connection` to use the SQL
  extension path. The Python binding owns its own connection in
  V1; that is the deliberate boundary.
  Target:
  - `spec-diff.md`
  - `plan.md`
  - `packages/bouncer-py/README.md`

- [D6] Root `README.md` adds one short example block that shows
  the three caller surfaces side by side: SQL extension for
  SQL-only callers, Python binding for typed Python callers, Rust
  wrapper for Rust callers. One snippet per surface; no full
  lease cycle.
  Target:
  - `spec-diff.md`
  - `plan.md`
  - `README.md`

- [D7] Adjacent Phase 009 review gaps stay out of named scope but
  remain tracked: the three missing in-transaction Python verb
  tests (`tx.inspect`, `tx.renew`, `tx.release`), the
  `bouncer-extension` first-class Rust integration test, the
  `BouncerError` non-lease error test, the `tx.execute`
  single-statement contract pin, and the cross-binding parity
  test. These can be folded in if the user explicitly asks; they
  are not silently included.
  Target:
  - `plan.md` (Out of scope but tracked section)

### Verdict

Phase 010 is open. The next correct move is a short Session B
review of `spec-diff.md` and `plan.md` before implementation.

## Review Round 001

### Reviewing

- the Phase 010 `plan.md`, `spec-diff.md`, and Decision Round 001
- the post-Phase-009 state of `packages/bouncer-py/python/bouncer/_bouncer.py`
  and `tests/test_bouncer.py` to verify the migration is safe
- the broader Honker-family Rust edition layout
  (`grep "^edition" .../Cargo.toml` across Bouncer, Honker, Knocker)
- the central question: is Phase 010's scope honest about what
  hardens the binding versus what is convenience-cleanup?

### Plan strengths

- Tight scope: five named items, none of which expand the public
  surface. "Out of scope but tracked" `[D7]` explicitly fences off
  adjacent Phase 009 review gaps so they cannot drift in silently.
- Each decision in Round 001 maps to a concrete code or doc target.
- Build order is concrete and ordered correctly: code first
  (move + remove), then tests, then edition, then docs.
- Migration is provably safe at plan time: all 11 existing Python
  tests use `with db.transaction() as tx:`, so moving
  `BEGIN IMMEDIATE` into `__enter__` cannot regress them. If a test
  breaks, that test was relying on a path the new contract forbids.
- The principle "honesty over convenience" is the right framing for
  both the eager-`BEGIN IMMEDIATE` removal and the `__del__`
  removal.

### Findings

- [F1] **The "family standard" Rust edition is ambiguous.** Plan
  `[D4]` aligns `bouncer-py` to `edition = "2021"` because
  `bouncer-core`, `bouncer-extension`, and `packages/bouncer` use
  it. But the broader Honker family is split: Honker
  (`honker`, `honker-rs`, `honker-node`) and Knocker
  (`packages/knocker`) all use `edition = "2024"`. So the choice is
  between two readings of "family standard":

  1. Local: align `bouncer-py` down to `2021` to match its three
     siblings inside Bouncer.
  2. Broader: align all four Bouncer crates up to `2024` to match
     Honker and Knocker.

  Neither is wrong. (1) is a one-line edit and matches what Phase
  010 promises. (2) is a multi-crate edit and would close an
  inter-repo edition drift but expands the phase. The plan should
  pick one explicitly rather than leave the ambiguity. (1) is
  cheaper and matches the named scope. (2) is the better long-term
  alignment but properly belongs in its own phase.

- [F2] **The "no implicit transaction" test is indirect.** Plan
  step 4 proposes
  `test_transaction_without_enter_does_not_open_sqlite_tx`, which
  asserts that `db.claim(...)` works after `db.transaction()`
  (no enter). That is an indirect signal: it works only because no
  `BEGIN IMMEDIATE` write lock is being held. A direct assertion
  uses two stdlib `sqlite3.Connection`s to the same file — if the
  first opened a write lock implicitly, a write from the second
  would block or error. Either keep the indirect test and add a
  direct one, or replace with the direct one.

- [F3] **README "use the SQL extension path" guidance needs a code
  example.** Plan `[D5]` says "short section telling users who
  already own a `sqlite3.Connection` to use the SQL extension path
  instead." Without a 5-line snippet showing
  `sqlite3.connect → enable_load_extension → load_extension →
  SELECT bouncer_bootstrap()`, the guidance is hand-wavy. Plan
  should pin a code example, not just prose.

- [F4] **`__del__` removal has a documented behavior change worth
  naming explicitly.** With `__del__` gone, a user who manually
  calls `tx.__enter__()` (without `with`) and then GCs the
  `Transaction` without calling `commit` or `rollback` leaks
  `transaction_active = True` on the native handle until the
  underlying `Bouncer` is also dropped. The next
  `db.transaction()` raises `BouncerError("transaction is active")`
  which is the loud-error behavior we want, but the plan should
  state this trade explicitly: convenience-rollback gone, replaced
  by a clear error at next-use. Honest, not silent.

- [F5] **Re-entry behavior on `Transaction.__enter__` is
  unspecified.** Plan says `__enter__` raises if `_entered` or
  `_finished` is true. That should be in `spec-diff.md` as part of
  the contract, not just in the plan. Specifically: a `Transaction`
  is single-use; entering twice or entering after explicit finish
  raises `BouncerError`.

- [F6] **`begin_transaction` failure inside `__enter__` leaves
  state ambiguous.** If `_native.begin_transaction()` raises (for
  example because another connection has an active write lock),
  what is `_entered` set to? Two reasonable answers:

  1. Stay `False` so the user can re-enter the same `Transaction`
     after the contention clears.
  2. Move to `True` regardless, forcing the user to construct a
     fresh `Transaction`.

  (1) is friendlier; (2) is more honest about "single-use" semantics.
  Plan should pin the answer.

- [F7] **Build order misses ROADMAP / CHANGELOG / SYSTEM updates.**
  Step 8 ends at `make test`. ROADMAP, CHANGELOG, and SYSTEM are
  listed under "Files likely to change" but not in the build
  order. Add an explicit step 9 covering these three docs at
  closeout, the way Phase 007 and Phase 009 closed.

- [F8] **Three adjacent gaps are small enough to fold in.** Of the
  five Phase 009 review items in the "Out of scope but tracked"
  section, three are tiny:

  - the three missing Python in-transaction verb tests
    (`tx.inspect`, `tx.renew`, `tx.release` happy path) — about 60
    lines, identical pattern to existing `test_full_lifecycle`
  - the `BouncerError` non-lease error test — about 10 lines
  - the `tx.execute` single-statement contract pin (one README
    sentence or a one-test reject)

  All three fit the "API honesty" theme: shipping verbs without
  direct tests is itself a kind of dishonesty. The named scope has
  intentionally excluded them, but they are small enough that
  excluding them makes Phase 010 narrower than its theme. Worth
  asking the user whether to fold them in. Two larger items —
  `bouncer-extension` first-class Rust integration test and
  cross-binding parity — correctly stay deferred.

- [F9] **`Transaction` is publicly exported but useless without a
  `Bouncer` + entry.** `packages/bouncer-py/python/bouncer/__init__.py`
  exports `Transaction` in `__all__`. After Phase 010 the class
  has no public constructor path users would reach for; everything
  goes through `db.transaction()`. Removing it from `__all__` (and
  optionally renaming to `_Transaction`) tightens the public
  surface. Stylistic, not blocking.

### Things checked and fine

- The migration is safe: all 11 Phase 009 Python tests use
  `with db.transaction() as tx:`, so moving the eager
  `BEGIN IMMEDIATE` into `__enter__` cannot break them.
- The native `transaction_active` flag still owns runtime
  exclusivity; only the *when* changes, not the *what*.
- `bouncer-core` stays the binding link target. The plan does not
  drift toward wrapping `packages/bouncer` or routing through the
  SQL extension.
- The five named items genuinely fit "API honesty" framing.
- Decision Round 001 explicitly fences adjacent gaps with `[D7]`.

### Verdict

The plan is implementable as written, and the contract decisions
in Round 001 are sound. Six of the nine findings are
"pin-the-detail" items that should land in `spec-diff.md` /
`plan.md` before code: `[F1]` (edition target), `[F2]` (direct vs
indirect test), `[F3]` (README code example), `[F4]` (documented
behavior change), `[F5]` (single-use contract), `[F6]`
(begin-failure state), `[F7]` (closeout doc updates).

`[F8]` is a scope question for the user: fold the three tiny
adjacent gaps in, or ship Phase 010 strictly as named. `[F9]` is
stylistic.

Recommended next step: a Decision Round 002 that resolves
`[F1]`-`[F7]` with one-sentence answers, then a user call on
`[F8]`. After that, implementation can proceed.

## Decision Round 002

### Responding to

- Review Round 001 `[F1]` through `[F9]`
- direct human instruction: fold `[F8]` in, drop `Transaction`
  from `__all__` per `[F9]`, accept the rest, proceed to
  implementation

### Decisions

- [D8] Accept `[F1]`. `bouncer-py` aligns to `edition = "2021"` to
  match its three siblings inside Bouncer. Broader-family
  alignment to `2024` (Honker, Knocker) is out of scope; if
  anyone wants the family-wide move, that is its own phase.
  Target:
  - `plan.md`
  - `packages/bouncer-py/Cargo.toml`

- [D9] Accept `[F2]` with the direct test. Replace the indirect
  `db.claim`-still-works test with a direct one that opens a
  second stdlib `sqlite3.Connection` to the same file and writes
  successfully under a short `busy_timeout`, proving the
  unentered `Transaction` did not hold an implicit write lock.
  Target:
  - `plan.md`
  - `packages/bouncer-py/tests/test_bouncer.py`

- [D10] Accept `[F3]`. The Python README "use the SQL extension
  path" section includes a five-line working snippet showing
  `sqlite3.connect → enable_load_extension → load_extension →
  SELECT bouncer_bootstrap()`, not just prose.
  Target:
  - `plan.md`
  - `packages/bouncer-py/README.md`

- [D11] Accept `[F4]`. `spec-diff.md` explicitly states the
  documented behavior change: removing `Transaction.__del__`
  means a user who manually calls `tx.__enter__()` (without
  `with`) and GCs the `Transaction` without `commit` or
  `rollback` leaks `transaction_active = True` on the native
  handle until the underlying `Bouncer` is dropped, at which
  point the next `db.transaction()` raises `BouncerError`.
  Honest fail-loud, not silent fix.
  Target:
  - `spec-diff.md`

- [D12] Accept `[F5]`. The single-use `Transaction` contract
  lands in `spec-diff.md`: `__enter__` raises `BouncerError` if
  the `Transaction` has already been entered or finished. A
  `Transaction` is single-use; reopen requires a fresh
  `db.transaction()` call.
  Target:
  - `spec-diff.md`
  - `packages/bouncer-py/python/bouncer/_bouncer.py`

- [D13] Accept `[F6]`. If `_native.begin_transaction()` raises
  inside `__enter__`, `_entered` remains `False` so the same
  `Transaction` instance can be re-entered after the contention
  clears. The friendlier of the two options.
  Target:
  - `spec-diff.md`
  - `packages/bouncer-py/python/bouncer/_bouncer.py`

- [D14] Accept `[F7]`. Build order step 9 covers the closeout
  doc updates: `ROADMAP.md`, `CHANGELOG.md`, `SYSTEM.md`.
  Target:
  - `plan.md`

- [D15] Accept `[F8]` with the three small adjacent gaps folded
  in:
  - three direct in-transaction Python verb tests
    (`tx.inspect` returning the live lease,
    `tx.renew` extending and rejecting wrong-owner,
    `tx.release` clearing the owner)
  - one `BouncerError` non-lease error test
    (SQL syntax error in `tx.execute` raises `BouncerError`)
  - the `tx.execute` single-statement contract pin: a one-line
    `packages/bouncer-py/README.md` note plus a regression test
    that documents the current rusqlite silent-drop behavior
    when given multi-statement SQL

  The two larger items (`bouncer-extension` first-class Rust
  integration test and cross-binding parity) stay deferred to
  separate phases.
  Target:
  - `plan.md`
  - `spec-diff.md`
  - `packages/bouncer-py/tests/test_bouncer.py`
  - `packages/bouncer-py/README.md`

- [D16] Accept `[F9]`. Drop `Transaction` from `__all__` and from
  the public re-export in `__init__.py`. Users only reach a
  `Transaction` through `db.transaction()`. Keep the class name
  unchanged for now (`Transaction`, not `_Transaction`) so type
  hints in user code do not break.
  Target:
  - `packages/bouncer-py/python/bouncer/__init__.py`

### Verdict

Phase 010 implementation can proceed. After implementation,
record the SHA in `commits.txt` and close the phase.
