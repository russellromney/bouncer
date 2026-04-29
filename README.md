# bouncer

Leases and coordination for the Honker stack.

## Summary

Bouncer is a small wrapper library for the question:

Who owns this named resource right now?

For the Honker stack, the point is not distributed consensus or
multi-node lock choreography. The point is giving a normal SQLite app a
boring, durable lease / ownership primitive in the same file it already
uses.

It should feel like a sibling to Knocker:

- `honker` stays the generic SQLite async substrate
- `bouncer-core` owns Bouncer's schema and SQLite operations
- `bouncer` bindings stay thin and simple

Bouncer should be the family's lease / fencing / leader-election
primitive. Over time, Honker itself may depend on it for scheduler
ownership and other single-machine coordination.

## What exists today

- repo-level docs for current intent and future direction
- a real `bouncer-core` crate
- a first Rust wrapper crate in `packages/bouncer`
- a first SQLite loadable-extension crate in `bouncer-extension`
- a SQLite schema bootstrap plus Rust `claim` / `renew` / `release` / `inspect`
- Rust tests that pin the initial lease semantics
- wrapper tests for explicit bootstrap and wrapper/core interoperability
- SQL function registration in the core plus SQL/Rust interop tests on one file
- transactional SQL mutators that can participate in an already-open caller transaction
- borrowed Rust mutators that can participate in a caller-owned
  transaction or savepoint without tripping nested transactions
- sanctioned wrapper-owned Rust transactions for atomic business writes
  plus lease mutations on one connection
- a first local-development Python package in `packages/bouncer-py`
  with explicit bootstrap, lease verbs, and a transaction context manager
- Python tests proving wrapper-owned transactions and SQL extension
  interop on the same database file
- wrapper convenience methods that stay thin and keep explicit-time control in the core

## V1 shape

- named resources
- claim / renew / release
- expiry
- fencing tokens
- inspect current owner

## Current SQL surface

The first SQLite-facing surface now exists via `bouncer-extension`:

- `bouncer_bootstrap()`
- `bouncer_claim(name, owner, ttl_ms, now_ms)`
- `bouncer_renew(name, owner, ttl_ms, now_ms)`
- `bouncer_release(name, owner, now_ms)`
- `bouncer_owner(name, now_ms)`
- `bouncer_token(name)`

The SQL surface stays explicit about time. Higher-level bindings can
offer convenience clocks later, but the SQLite-facing contract should
not hide `now_ms` inside the extension.

Mutating SQL helpers can also participate in a caller-owned explicit
transaction, so a business write and a Bouncer lease mutation can commit
or roll back together on one connection.

## Choosing a surface

Three caller surfaces share one Bouncer-owned schema and lease
semantics. Pick the one that matches how you already use SQLite. All
three operate on the same `bouncer_resources` table and can coexist on
one database file.

**SQL extension** — for any SQLite client that wants Bouncer
semantics on a connection it already owns. Load
`libbouncer_ext.{dylib,so,dll}` and call the `bouncer_*` SQL functions:

```sql
SELECT bouncer_bootstrap();
SELECT bouncer_claim('scheduler', 'worker-a', 30000, 1700000000000);
SELECT bouncer_owner('scheduler', 1700000000000);
```

**Python binding** — for Python callers who want a typed,
binding-owned SQLite connection:

```python
import bouncer
db = bouncer.open("app.sqlite3")
db.bootstrap()
result = db.claim("scheduler", "worker-a", ttl_ms=30_000)
```

**Rust wrapper** — for Rust callers, with a sanctioned transaction
handle and savepoint surface:

```rust
let mut db = bouncer::Bouncer::open("app.sqlite3")?;
db.bootstrap()?;
let tx = db.transaction()?;
tx.claim("scheduler", "worker-a", Duration::from_secs(30))?;
tx.commit()?;
```

## Non-goals

- distributed consensus
- waiting queues
- fairness
- elaborate leader election

## Intent

Bouncer exists for the single-machine SQLite stack:

- app processes on one host
- one SQLite file
- background workers, schedulers, migrations, importers
- no Redis, Consul, ZooKeeper, or etcd just to answer "who owns this?"

If it cannot make that use case materially simpler, it should stay a
small internal primitive rather than grow into a bigger product.

## Intent artifacts

- `SYSTEM.md`
  current English model of the system
- `ROADMAP.md`
  remaining product and implementation work
- `CHANGELOG.md`
  completed work summary
- `.intent/phases/.../spec-diff.md`
  intended semantic change for one phase
- `.intent/phases/.../plan.md`
  implementation reasoning for one phase
- `.intent/phases/.../reviews_and_decisions.md`
  review history plus explicit responses for one phase

## Repo shape

- `bouncer-core`
  Rust core that owns schema and SQLite operations
- `bouncer-extension`
  SQLite loadable extension / shared SQL surface
- `packages/bouncer`
  thin Rust binding surface
- `packages/bouncer-py`
  thin Python binding surface
