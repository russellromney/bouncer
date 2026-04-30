# Spec Diff

Phase: 014 — docs as safety rails

Session:
- A

## What changes

- Bouncer gains a documentation-and-proof phase that turns the proved
  behavior from Phases 011–013 into practical operator guidance and
  adds one direct test matrix for the remaining load-bearing doc claim:
  pragma neutrality.
- This phase is about caller safety rails, not new lease semantics.
  The docs should help users distinguish:
  - lease busy versus SQLite lock/busy/locked failure
  - autocommit versus caller-owned transaction behavior
  - `BEGIN IMMEDIATE` versus deferred `BEGIN`
  - wrapper-owned connection flow versus caller-owned SQLite flow
  - Bouncer's fencing token versus downstream stale-actor enforcement
- This phase pins a real pragma-neutrality contract and proves it with
  a matrix rather than by documentation alone. The contract is:
  Bouncer does not set, rewrite, or normalize caller-owned SQLite
  pragma policy as a side effect of bootstrap or lease operations.
- For this phase, the load-bearing pragma-neutrality rows cover five
  caller-visible settings that make the contract concrete without
  pretending to cover every SQLite knob:
  - file/persistent policy:
    - `journal_mode`
    - `synchronous`
  - connection-local policy:
    - `busy_timeout`
    - `locking_mode`
    - `foreign_keys`
- Those five pragmas are part of the contract matrix, not just doc
  text.
- The matrix must prove pragma neutrality across the sanctioned public
  surfaces that might otherwise hide incidental connection policy:
  - core direct Rust helpers
  - in-process SQL extension registration and SQL function calls
  - Rust wrapper bootstrap, borrowed path, transaction path, and typed
    savepoint path
- Each matrix row sets concrete pragma values first, runs the Bouncer
  operation, then re-reads the pragma state and asserts it is
  unchanged. File-persistent pragma rows should verify both the active
  connection and a fresh connection on the same database file.
- The docs should turn the current multi-surface story into a clear
  "which surface should I use?" guide:
  - Rust wrapper for typed Rust callers
  - SQL extension for callers who already own the SQLite connection
  - Python binding for Python callers who want a binding-owned path
- This phase may add small doc examples, troubleshooting tables, short
  cross-links, and test-only helper code where they materially reduce
  likely misuse, but it does not add new production behavior.

## What does not change

- No new lease semantics.
- No new bindings.
- No migration story.
- No new production pragma policy. This phase proves neutrality; it does
  not introduce defaults or normalization.
- No hidden policy around `busy_timeout`, `journal_mode`, or retry
  loops.

## How we will verify it

- Update the root docs (`README.md`, `SYSTEM.md`, and possibly package
  READMEs) so the Phase 012/013 safety-critical behavior is described
  consistently.
- Add a dedicated pragma-neutrality matrix that proves the current docs
  are not over-claiming. At minimum it should cover:
  - `journal_mode`
  - `synchronous`
  - `busy_timeout`
  - `locking_mode`
  - `foreign_keys`
  - core `bootstrap_bouncer_schema`
  - one direct core autocommit mutator
  - one direct core `*_in_tx` mutator inside caller-owned transaction
  - SQL function registration plus `bouncer_bootstrap()`
  - one SQL mutator in autocommit mode
  - one SQL mutator inside caller-owned deferred transaction/savepoint
  - wrapper `Bouncer::bootstrap()`
  - wrapper borrowed-path mutator
  - wrapper `Bouncer::transaction()`
  - wrapper typed savepoint path
- Add one short troubleshooting/help section that answers the likely
  user questions:
  - "Why did I get Busy versus SQLite busy/locked?"
  - "When should I use `BEGIN IMMEDIATE`?"
  - "Who should own `busy_timeout` and journal mode?"
  - "What do I have to do with the fencing token?"
  - "Which surface should I use if I already own the connection?"
- Build docs and run the existing test suite if any examples or code
  comments change in a way that touches production-facing guidance.
- Run the new matrix plus the existing Rust suite.

## Notes

- The point is not to make the docs longer. The point is to make the
  dangerous edges harder to misunderstand.
- "Pragma-neutral" in this phase means "Bouncer leaves caller-selected
  pragma values alone." In this phase the proved cases are
  `journal_mode`, `synchronous`, `busy_timeout`, `locking_mode`, and
  `foreign_keys`. It does not mean "every SQLite pragma has been
  exhaustively enumerated forever." The contract is the pinned matrix
  above.
