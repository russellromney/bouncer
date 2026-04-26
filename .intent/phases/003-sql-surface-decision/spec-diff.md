What changes:

- Bouncer gets a decision phase for whether it should expose a SQLite
  SQL/loadable-extension surface after the Rust binding.
- The phase may either:
  - define a minimal SQL surface and the proof required for it, or
  - explicitly defer SQL helpers as the wrong next move.
- The phase must decide whether SQL exposure strengthens Bouncer's
  single-machine SQLite story or just creates a second public surface
  too early.

What does not change:

- Phase 003 does not change Phase 001 lease semantics.
- Phase 003 does not add a Python, Node, or other non-Rust binding.
- Phase 003 does not expand Bouncer into a scheduler or workflow system.
- Phase 003 does not commit Honker to depending on Bouncer yet.

How we will verify it:

- The phase ends with a clear yes/no decision on the SQL surface.
- If yes, the resulting plan names the smallest SQL contract worth
  shipping and the tests/docs required to keep it honest.
- If no, the resulting plan says plainly why the SQL surface is being
  deferred and what should happen next instead.
