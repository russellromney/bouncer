What changes:

- Bouncer gains its first thin Rust binding crate in `packages/bouncer`.
- The binding wraps the Phase 001 core contract rather than
  reimplementing lease semantics.
- The binding provides a small ergonomic API for opening a SQLite
  database, bootstrapping Bouncer's schema, and calling `inspect`,
  `claim`, `renew`, and `release`.
- The binding hides explicit `now_ms` injection behind normal
  system-time defaults for everyday callers while leaving the lower-level
  core crate available for deterministic tests and advanced use.

What does not change:

- The Phase 001 lease semantics do not change.
- Bouncer does not gain a loadable-extension SQL surface yet.
- Bouncer does not add background renewal, lease guards, waiting queues,
  or async orchestration.
- The binding does not invent a second state machine on top of
  `bouncer-honker`.

How we will verify it:

- A caller can open a SQLite database through the binding and
  successfully claim, inspect, renew, and release a resource.
- The binding and `bouncer-honker` interoperate against the same
  database file.
- The binding maps Phase 001 core results into a small public API
  without changing the underlying semantics.
- Rust tests pin the wrapper behavior before any non-Rust binding is
  attempted.
