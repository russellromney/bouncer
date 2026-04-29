# Bouncer Roadmap

## Summary

Bouncer should be the sharpest "who owns this right now?" primitive for the Honker family.

Its job is not to become a scheduler or workflow system. Its job is to provide a durable lease with expiry and fencing in the same SQLite file the app already uses.

## Intent artifacts

- `SYSTEM.md` is the current English model of Bouncer.
- `CHANGELOG.md` records what has landed.
- future meaningful changes should add `.intent/` records with spec diffs, plans, and `reviews_and_decisions.md`.

## Current status

The repo now has a real Phase 001 Rust core:

- `bouncer-core` owns the first SQLite schema
- the core contract exposes `inspect`, `claim`, `renew`, and `release`
- fencing tokens are monotonic across expiry, release, and re-claim
- Rust tests pin the current semantics
- a first Rust wrapper crate now exists in `packages/bouncer`
- the wrapper stays thin, keeps bootstrap explicit, and leaves time-ordering concerns out of scope
- a first SQLite loadable-extension crate now exists in `bouncer-extension`
- the first `bouncer_*` SQL surface reuses `bouncer-core` semantics and keeps `now_ms` explicit
- SQL/Rust interop is now proven on one database file
- SQL mutators now participate in caller-owned explicit transactions and savepoints instead of failing with SQLite's nested-transaction error
- deferred multi-connection writer contention is now pinned as a lock/busy failure in the in-transaction SQL path
- borrowed Rust mutators now follow the same transaction model as the SQL extension instead of tripping nested-transaction errors on caller-owned transactions
- the core now exposes explicit public `*_in_tx` helpers with a fail-fast transaction-state guard
- the wrapper now exposes a sanctioned `Bouncer::transaction()` handle
  with checked `BEGIN IMMEDIATE` semantics and same-wrapper exclusivity
- the wrapper transaction handle now exposes a sanctioned `savepoint()`
  nested boundary with tested claim/renew/release/inspect behavior
- wrapper transaction and savepoint commits are now proven visible from
  fresh connections after the outer transaction commits
- wrapper semantic-stress tests now use explicit-time core helpers
  instead of sleep-based expiry waits where practical
- `packages/bouncer/src/lib.rs` has been split so the public wrapper
  surface stays small and the test modules carry the test bulk
- wrapper and system docs now name the recommended default surfaces:
  `Bouncer`, `Bouncer::transaction()`, and `BouncerRef`
- a first Python binding now exists in `packages/bouncer-py`
- the Python binding exposes explicit `bootstrap()`, `inspect`, `claim`,
  `renew`, `release`, and a transaction context manager
- Python transaction tests prove business writes and lease mutations
  commit and roll back together
- Python cross-surface tests prove the binding and SQLite extension share
  one database-file contract
- the Python `Transaction` is honestly context-manager-first;
  `BEGIN IMMEDIATE` opens inside `__enter__`, the `Transaction` is
  single-use, and pre-`__enter__` verb calls fail loudly
- `Transaction.__del__` is gone; the native `Drop for NativeBouncer` is
  the only remaining transaction safety net
- `bouncer-py`'s Rust edition matches its in-repo siblings (2021)
- the package and root READMEs now document the three caller surfaces
  (SQL extension, Python binding, Rust wrapper) and tell
  `sqlite3.Connection`-owning Python callers to use the SQL extension
  path

The intended model is:

- `honker`
  generic queue / wake / retry substrate
- `bouncer-core`
  Bouncer-specific schema and SQLite contract
- `bouncer-extension`
  shared SQLite-facing SQL boundary
- `bouncer`
  thin language bindings

## Next build steps

The next Bouncer work should harden the Rust/SQLite primitive, not add
more bindings. Python is useful as an example binding and cross-surface
proof, but the primary correctness surfaces are `bouncer-core`,
`bouncer-extension`, and the Rust wrapper.

1. **Phase 011 — deterministic invariant runner.**
   Add a deterministic core-level operation runner over claim, renew,
   release, inspect, owner, and token. Generate many explicit-time
   operation sequences across resources and owners. Assert invariants:
   no two live owners for one resource, fencing tokens never decrease,
   release never resets token state, wrong owners cannot renew/release,
   expiry makes takeover possible, and rejected operations do not mutate
   state. This is the first small DST-shaped harness: seeded operations,
   injected time, replayable failures, and property assertions, without
   yet adding fault injection, VFS shims, or a shared family simulator.
2. **Phase 012 — SQLite behavior matrix.**
   Exhaustively pin the lease behavior under `BEGIN`, `BEGIN IMMEDIATE`,
   savepoints, autocommit, two connections, zero `busy_timeout`, nonzero
   `busy_timeout`, lock contention, and practical SQLite settings such
   as `journal_mode` (`WAL`, `DELETE`), `synchronous`, `locking_mode`,
   and extension loading. The goal is to clearly separate "lease busy"
   from "SQLite writer lock busy" and prove Bouncer does not accidentally
   depend on one happy-path SQLite setup.
3. **Phase 013 — schema and data-integrity hardening.**
   Decide and test behavior for invalid/manual rows, schema drift, old
   schema versions, token near-overflow, bad `ttl_ms`, huge timestamps,
   unusual names/owners, and partial application edits. Make impossible
   rows either impossible by constraint or loud by error.
4. **Phase 014 — docs as safety rails.**
   Add troubleshooting and safety docs for the cases users will hit:
   lease busy vs SQLite busy vs timeout, `BEGIN IMMEDIATE` guidance,
   fencing-token obligations, pragma policy, and which surface to use
   when the caller owns the SQLite connection.

## Future proposals

### Nested wrapper savepoints

The Rust wrapper now has one sanctioned savepoint level through
`Transaction::savepoint()`. Nested savepoints are a plausible future
ergonomic surface, especially if a binding wants nested context
managers or an ORM/framework integration needs local rollback scopes
inside a larger transaction.

Do not add this just because SQLite supports it. Add it when a caller
story needs it, and keep the same terminal handle shape:

- opening a nested boundary borrows the parent savepoint mutably
- `commit(self)` releases the nested savepoint
- `rollback(self)` rolls back to and releases the nested savepoint
- outer rollback still discards all nested work

### Language bindings strategy

Bouncer should match Honker's binding footprint where a language's
stdlib SQLite extension-load path is awkward enough that hiding it
inside a typed wrapper carries real value. Don't preemptively build
bindings for languages where the SQL extension is one-line to load
and the four `bouncer_*` SQL functions are easy to call directly.

Each binding lands when a real consumer in that language asks. This
section exists so future-us doesn't accidentally build a Node binding
just because Honker has one.

Currently shipped:

- **Rust** (`packages/bouncer`) — links `bouncer-core` directly. The
  binding's value beyond the SQL extension is compile-time `&mut self`
  exclusivity on `Bouncer::transaction()` and the `Savepoint` handle.
  This is real and not replicable in raw SQL.
- **Python** (`packages/bouncer-py`) — PyO3 binding linking
  `bouncer-core`. Hides stdlib `sqlite3`'s 3-step
  `enable_load_extension(True) → load_extension(path) →
  enable_load_extension(False)` dance, which sits behind a
  Python-level permission gate. Also adds typed dataclass results,
  context-manager transactions, and `now_ms` injection.

Likely future bindings, in priority order if and when a consumer
asks:

1. **Go** — highest awkwardness in the family. `mattn/go-sqlite3`
   requires registering a custom `sqlite3` driver with a `ConnectHook`
   that loads the extension on every pool connection, and Honker's
   `honker-go` uses an atomic counter to mint unique driver names so
   multiple databases in one process don't collide. Replicating that
   per Bouncer caller is the worst extension-load story we've seen.
2. **Ruby** — same 3-step `enable_load_extension(true) →
   load_extension(path) → enable_load_extension(false)` dance Python
   has. Honker's `honker-ruby` hides it; a Bouncer-Ruby binding would
   too.
3. **Elixir** — same 3-step dance via Exqlite
   (`Sqlite3.enable_load_extension → SELECT load_extension(?) →
   enable_load_extension(false)`). Honker's `honker-ex` hides it.
4. **Node** — Honker's `honker-node` is FFI-linked via napi-rs, so
   no `load_extension` involved. A Bouncer-Node binding following the
   same FFI pattern would mostly add typed result objects and tx
   wrapping, not awkwardness-hiding. Lower marginal value than Python
   had, but worth Honker-stack parity.
5. **Bun** — `raw.loadExtension(path)` is a one-liner. The only reason
   to build a binding is Honker-stack parity. Lowest priority.

Languages we will not pursue without a strong consumer:

- **C++** — niche. The Honker family's C++ pattern is Zig + direct C
  SQLite API; that style does not benefit from the kind of typed
  wrapping Bouncer offers.
- **Java, .NET, Swift, etc.** — same posture. Build only when a real
  consumer arrives.

Why this differs from Honker's posture:

Honker has 7 bindings because Honker's surface is large and
shape-hostile to raw SQL: queues, streams, locks, rate-limit, tasks,
fanout; rich state with Job objects, payload deserialization, ack /
nack semantics, retry config. Per-language wrapping pays off because
hand-rolling every one of those verbs against the SQL extension is
real work. Bouncer has four verbs, single-scalar SQL returns, and a
four-field result struct. The marginal value of a binding is small
unless the language's stdlib SQLite path is awkward enough to make
the extension dance the dominant cost. Add bindings selectively, not
by default.

### DST-forward (deterministic simulation testing)

Honker and its siblings (bouncer-core, future queue/retry/scheduler
primitives) should be testable under deterministic simulation: every
source of non-determinism flows through a seam the test harness
controls, and the entire system is replayable from a seed. Inspired
by TigerBeetle's VOPR, FoundationDB's simulator, sled's deterministic
test harness, and Antithesis.

The bar:

- All time is injected. Already true at the `bouncer-core` core
  (`now_ms` parameter on every function). The wrapper layer must
  preserve this — production callers see a `Clock` default, tests see
  whatever the harness wants.
- All randomness flows through a seeded source. Added per-sibling as
  needed (none today).
- All SQLite I/O is interceptable. Needs a VFS shim or rusqlite hook
  so the harness can inject `SQLITE_BUSY`, `SQLITE_FULL`, partial
  writes, fsync drops, and torn pages.
- All operation scheduling is controllable. A generator that produces
  a sequence of `(operation, conn, args)` with seeded selection, run
  by a single-threaded scheduler that can permute order across
  simulated processes.
- Properties replace scenarios. Invariants like "fencing token never
  decreases," "no two live owners simultaneously," "released →
  reclaimable," "expired → takeover succeeds + token++" run across
  millions of seeds.
- Bug minimization. When a property fails, the seed reproduces, and a
  shrinker reduces to the smallest failing trace.

What lives where:

- Bouncer should start with a small local deterministic runner because
  the core already has the most important seam: explicit `now_ms`.
- A later shared Honker-family harness can extract the useful pieces
  once Bouncer proves the shape: clock seam, op generator, scheduler,
  VFS shim, and property runner.
- Each sibling (bouncer-core, future queue/retry/scheduler) provides
  its own operation generator and invariant set.
- Production code stays unchanged. DST is a test-time superpower, not
  a runtime cost.

Implications for current decisions:

- The core already has the important seam (`now_ms` on every function).
- A future wrapper phase may add an injectable clock seam if the
  deterministic-simulation investment becomes concrete. Phase 002 can
  stay thinner as long as it does not hide or replace the core's
  explicit-time contract.
- Reading time from inside SQLite (e.g. `unixepoch()`) defeats the
  injection seam and is therefore inconsistent with this direction.
  Keep all time on the Rust side, behind a `Clock` trait or
  equivalent.

Out of scope for this proposal:

- Multi-machine simulation. Cinch is single-machine; fencing token +
  lease semantics are the cross-machine story.
- Replacing real concurrency tests entirely. DST complements stress
  tests, doesn't replace them.
- OS/network-level fault injection. Lives elsewhere if ever needed.

This is a meaningful infrastructure investment, but it should begin
small. Bouncer should prove the lightweight version first; the family
can extract a shared simulator only after the local version catches
real bugs or proves enough value to be worth centralizing.

## V1 nouns

- resource
- owner
- lease
- fencing token

## Success criteria

- one current owner per named resource
- expiry is durable and inspectable
- fencing token increments on successful claim
- bindings do not reimplement semantics
