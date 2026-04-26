What changes:

- Bouncer gains its first thin Rust binding crate in `packages/bouncer`.
- The binding wraps the Phase 001 core contract rather than
  reimplementing lease semantics.
- The binding provides a small ergonomic API for opening a SQLite
  database, explicitly bootstrapping Bouncer's schema, and calling
  `inspect`, `claim`, `renew`, and `release`.
- The binding uses ordinary system-time defaults for its convenience
  methods, while leaving explicit-time control in `bouncer-honker`.
- The binding re-exports the core lease/result shapes rather than
  inventing a second public state machine.
- The binding supports both an owned open path and a borrowed wrapper
  around an existing `rusqlite::Connection`.
- The binding does not use wall clock as an ordering primitive.
  Time remains lease-expiry bookkeeping; correctness and stale-actor
  safety still flow through SQLite write serialization and the fencing
  token.

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
- Wrapper methods called before `bootstrap()` fail cleanly rather than
  bootstrapping implicitly or panicking.
- The binding and `bouncer-honker` interoperate against the same
  database file.
- The binding maps Phase 001 core results into a small public API
  without changing the underlying semantics.
- Interop is proven across separate SQLite connections to the same file,
  including fencing-token monotonicity across wrapper/core calls.
- The wrapper's bootstrap path is explicit and idempotent.
- Rust tests pin the wrapper's bootstrap/error/interoperability behavior
  before any non-Rust binding is attempted.
