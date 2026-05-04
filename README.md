# bouncer

Leases, ownership, and fencing tokens for SQLite apps.

## What is this?

Bouncer answers one small question: who owns this named resource right
now?

It gives a normal SQLite application a durable lease primitive in the
same database file it already uses. That makes it useful for things
like:

- one scheduler should run at a time
- one background worker should own a shard
- one importer should hold exclusive ownership while it works
- one process should be leader for some local responsibility

Bouncer is not a queue, a workflow engine, or distributed consensus. It
is a small SQLite state machine with expiry and fencing tokens.

## Status

Bouncer is real and heavily tested, but still early. Today it has a
Rust core, a SQLite loadable extension, a Rust wrapper, a small Python
convenience layer, and direct proof for semantics, SQLite behavior,
schema hardening, pragma-neutrality, and user-shaped acceptance.

What it does not have yet is a migration story, a broad binding
strategy, a package-manager-first install story across every surface, or
any claim to distributed consensus or multi-machine coordination.

The clearest way to think about it today is: SQL extension as the base
interoperability surface, Rust wrapper as the main convenience layer,
and Python as the easiest way to try it from Python.

## How do I try it?

From the repo root:

```bash
make test-rust
make build-ext
make test-python
```

If you want the quickest hands-on path, start with:

- [bouncer-extension/examples/basic_claim.sql](/Users/russellromney/Documents/Github/bouncer/bouncer-extension/examples/basic_claim.sql)
- [packages/bouncer/examples/basic_claim.rs](/Users/russellromney/Documents/Github/bouncer/packages/bouncer/examples/basic_claim.rs)
- [packages/bouncer-py/examples/basic_claim.py](/Users/russellromney/Documents/Github/bouncer/packages/bouncer-py/examples/basic_claim.py)

## How do I install it?

Today the cleanest way to use Bouncer is still from source.

- **SQL extension**: build `bouncer-extension` and load the resulting
  `libbouncer_ext.{dylib,so,dll}` into the SQLite connection you already
  own
- **Rust wrapper**: use the in-repo crate today
- **Python**: use the local development package if you want the easiest
  way to try it from Python

From the repo root, the extension build is:

```bash
cargo build -p bouncer-extension --release
```

The output artifact will be under `target/release/`. Tagged GitHub
releases also build and attach prebuilt extension binaries for the main
platforms.

## Why does it exist?

Plenty of apps already have the hard part they need: one SQLite file
shared by a few threads, processes, jobs, or local daemons.

What they usually do not have is a clean answer to:

- "who owns this job runner right now?"
- "how do I fail over if the owner dies?"
- "how do I stop a stale actor from continuing work after takeover?"

The usual alternatives are all annoying in different ways:

- ad hoc lock tables with fuzzy semantics
- PID files and temp files
- hand-rolled "heartbeat" rows
- dragging in Redis or a bigger coordination system for a local app

Bouncer exists to make that boring.

## Why not just a lock table?

A lock table can be enough for a rough internal tool. Bouncer exists for
the parts that usually stay under-specified: what reclaim means after
expiry, how stale actors are fenced off, how lease conflict differs from
SQLite `BUSY` / `LOCKED`, how the lease mutation should participate in
caller-owned transactions and savepoints, and what happens when the
persisted state is invalid or has drifted from the expected schema.

The happy path can be a handful of SQL statements. The edge-case
contract is the larger part of the problem.

## Why would I use it?

Use Bouncer when:

- you already use SQLite
- you want one durable owner of a named resource
- expiry and takeover should be explicit
- stale-actor protection matters
- you want to keep coordination in the same file as the rest of the app

Do not use Bouncer when:

- you need cross-machine consensus
- you need fairness or waiting queues
- you need a job system rather than a lease primitive
- you want Bouncer to enforce downstream stale-write rejection by itself

## Limitations

Bouncer is intentionally narrow. It is for a single machine and one
SQLite file. It does not have a migration story today, it does not try
to provide fairness or waiting-queue semantics, and it is not
distributed consensus. Downstream token enforcement is still the
caller’s job.

## Design at a glance

Bouncer stores lease state in one table, `bouncer_resources`. Each
resource row tracks a `name`, an `owner`, a monotonic `token`, and
`lease_expires_at_ms`.

The contract is simple but specific: the first successful claim creates
the row, a live lease blocks a second claim, release and expiry clear
live ownership, reclaim after release or expiry increments the fencing
token, and tokens never go backwards.

That last part matters. The token is how you protect downstream systems
from stale actors. If worker A loses the lease and worker B takes over,
worker B gets a larger token. Your downstream side-effect boundary has
to carry and compare that token if you want stale writes rejected
outside SQLite.

## Surfaces

All shipped surfaces share one schema and one lease state machine.

| Surface | Who owns the SQLite connection? | Best for |
|---|---|---|
| SQL extension | caller | any app that already owns the SQLite connection |
| Rust `Bouncer` | Bouncer | normal Rust apps that want typed results |
| Rust `BouncerRef` | caller | Rust code that already owns a `rusqlite::Connection` or transaction |
| Python `bouncer` | Bouncer | trying Bouncer from Python without hand-loading the extension |

The shortest rule is:

- if **you** already own the SQLite connection, use the **SQL extension**
- if you are in **Rust** and want a wrapper-owned path, use **`Bouncer`**
- if you are in **Rust** and already own the connection, use **`BouncerRef`**
- if you are in **Python** and just want an easy way to try Bouncer, use the
  **Python binding**

### SQL extension

Use this when you already own the SQLite connection and want Bouncer to
participate on that exact connection.

```sql
SELECT bouncer_bootstrap();
SELECT bouncer_claim('scheduler', 'worker-a', 30000, 1700000000000);
SELECT bouncer_owner('scheduler', 1700000000000);
SELECT bouncer_token('scheduler');
```

The SQL surface keeps time explicit. You pass `now_ms` yourself.

See also:

- [bouncer-extension/examples/basic_claim.sql](/Users/russellromney/Documents/Github/bouncer/bouncer-extension/examples/basic_claim.sql)
- [bouncer-extension/examples/transactional_claim.sql](/Users/russellromney/Documents/Github/bouncer/bouncer-extension/examples/transactional_claim.sql)

### Rust wrapper

Use this when you are writing Rust and want typed results plus a
sanctioned transaction/savepoint surface.

```rust
use std::time::Duration;

use bouncer::{Bouncer, ClaimResult};

let db = Bouncer::open("app.sqlite3")?;
db.bootstrap()?;

match db.claim("scheduler", "worker-a", Duration::from_secs(30))? {
    ClaimResult::Acquired(lease) => {
        println!("got token {}", lease.token);
    }
    ClaimResult::Busy(current) => {
        println!("currently owned by {}", current.owner);
    }
}
# Ok::<(), Box<dyn std::error::Error>>(())
```

See also:

- [packages/bouncer/examples/basic_claim.rs](/Users/russellromney/Documents/Github/bouncer/packages/bouncer/examples/basic_claim.rs)
- [packages/bouncer/examples/transactional_claim.rs](/Users/russellromney/Documents/Github/bouncer/packages/bouncer/examples/transactional_claim.rs)

`Bouncer` is the default Rust surface. `BouncerRef` is the lower-level
integration surface when your code already owns the `rusqlite::Connection`
or the current transaction/savepoint boundary.

### Python binding

Use this when you want the easiest way to try Bouncer from Python and
you are happy letting the binding own the SQLite connection.

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

See also:

- [packages/bouncer-py/examples/basic_claim.py](/Users/russellromney/Documents/Github/bouncer/packages/bouncer-py/examples/basic_claim.py)
- [packages/bouncer-py/examples/transactional_claim.py](/Users/russellromney/Documents/Github/bouncer/packages/bouncer-py/examples/transactional_claim.py)

This is intentionally not the main integration surface for Python. It is
a thin convenience layer and an easy try-it-out path. If you already own
a `sqlite3.Connection`, use the SQL extension on the connection you
already manage.

## How do I use it?

The happy path is:

1. open the database
2. call `bootstrap()` once per file
3. claim a named resource with an owner and TTL
4. renew while the owner is still alive
5. release when the owner is done
6. on takeover, use the new fencing token downstream

If your app needs one business write plus one lease mutation to commit
or roll back together, use a caller-owned transaction boundary.

Rust wrapper example:

```rust
use std::time::Duration;

use bouncer::{Bouncer, ClaimResult};
use rusqlite::params;

let mut db = Bouncer::open("app.sqlite3")?;
db.bootstrap()?;

let tx = db.transaction()?;
tx.conn().execute(
    "INSERT INTO jobs(payload) VALUES (?1)",
    params!["work"],
)?;

match tx.claim("scheduler", "worker-a", Duration::from_secs(30))? {
    ClaimResult::Acquired(_) => tx.commit()?,
    ClaimResult::Busy(_) => tx.rollback()?,
}
# Ok::<(), Box<dyn std::error::Error>>(())
```

Python binding example:

```python
with db.transaction() as tx:
    tx.execute("INSERT INTO jobs(payload) VALUES (?)", ["work"])
    claim = tx.claim("scheduler", "worker-a", ttl_ms=30_000)
    if not claim.acquired:
        tx.rollback()
```

If you already own a `sqlite3.Connection` in Python, use the SQL
extension instead of the Python binding. The Python binding is meant to
be the easy try-it-out path, not a second way to drive an already-open
SQLite handle.

## How does the transaction story work?

There are two modes. In autocommit, Bouncer owns the transaction and
opens `BEGIN IMMEDIATE`. Inside a caller-owned transaction or savepoint,
Bouncer reuses the boundary you already opened. That means it does not
invent a separate transaction model.

The main practical implication is that deferred `BEGIN` and
`BEGIN IMMEDIATE` are different:

- `BEGIN IMMEDIATE` takes writer intent up front
- deferred `BEGIN` can fail later during lock upgrade

So if you need predictable atomic business-write-plus-lease behavior,
open the outer transaction with `BEGIN IMMEDIATE`.

## Safety rails

These are the sharp edges worth remembering.

### Lease busy is not SQLite busy/locked

If another owner already holds a live lease, Bouncer returns a lease
busy result: Rust gives `ClaimResult::Busy`, Python gives
`acquired=False`, and SQL returns `NULL` from `bouncer_claim(...)`.
That is different from SQLite `BUSY` / `LOCKED`, which means writer
contention or deferred lock-upgrade failure.

### Bouncer is pragma-neutral

Bouncer does not set or normalize your connection policy. The current
proved set is `journal_mode`, `synchronous`, `busy_timeout`,
`locking_mode`, and `foreign_keys`. Set them yourself before calling
`bootstrap()` or before handing a connection to `BouncerRef`.

### Bootstrap is strict

If `bouncer_resources` already exists with the wrong shape, bootstrap
fails loudly with `SchemaMismatch`. Bouncer is not a migration engine
and does not silently accept drifted schema.

### Fencing tokens matter only if you carry them

Bouncer guarantees monotonic tokens. It cannot make your downstream
systems check them for you. If stale-actor protection matters beyond
SQLite, carry the token to the place where side effects happen.

## FAQ

### Why not just use a lock table?

You can, if the happy path is all you need. Bouncer exists for the
harder parts: expiry/reclaim semantics, monotonic fencing tokens, lease
busy versus SQLite busy/locked, transaction/savepoint participation, and
strict handling of drifted or invalid persisted state.

### Why not just use Redis or Postgres advisory locks?

If your app already runs Redis or Postgres and that solves the right
problem for you, that can be a fine choice. Bouncer is for the narrower
case where SQLite is already the datastore and you want the lease state
in the same file as the rest of the application.

### What happens if the owner dies?

The lease expires, another caller can reclaim it, and the next
successful owner gets a larger fencing token. That token is what lets
downstream systems reject stale work from the old owner if you carry it
through your side-effect boundary.

### When should I use SQL versus the Rust wrapper?

Use the SQL extension when you already own the SQLite connection. Use
the Rust wrapper when you want a friendlier typed Rust API on a
wrapper-owned connection. Use `BouncerRef` when Rust already owns the
connection and you want Bouncer to participate in that exact SQLite
boundary.

### Is Python a primary surface?

Not really. The Python package is a thin convenience layer and an easy
way to try Bouncer from Python. The main product surfaces are the SQL
extension and the Rust wrapper.

## Current proof

The repo now has direct proof for core lease semantics, deterministic
invariant coverage, SQLite busy/locked versus lease-busy behavior,
strict schema-drift rejection and invalid-row hardening,
pragma-neutrality across the main public surfaces, and user-shaped
acceptance journeys on one real database file.

That acceptance layer includes fresh bootstrap plus first claim,
independent second-caller busy, release/reclaim token increase,
deterministic expiry/reclaim token increase, transaction atomic
visibility, loud drifted-schema bootstrap failure, and direct Rust
wrapper / SQL extension / Python binding interop.

Python is useful here mostly as a convenience/demo layer and as a
cross-surface proof surface. The core product story is still the SQL
extension plus the Rust wrapper.

## Repository map

- [bouncer-core](/Users/russellromney/Documents/Github/bouncer/bouncer-core)
  Rust core owning schema and lease semantics
- [bouncer-extension](/Users/russellromney/Documents/Github/bouncer/bouncer-extension)
  SQLite loadable extension
- [packages/bouncer](/Users/russellromney/Documents/Github/bouncer/packages/bouncer)
  Rust wrapper
- [packages/bouncer-py](/Users/russellromney/Documents/Github/bouncer/packages/bouncer-py)
  Python binding

## Development

```bash
make test-rust
make test-python
make test
```

## Origins

Bouncer started alongside other SQLite infrastructure work, but the
primitive stands on its own. If you have a single-machine SQLite app and
need durable ownership with fencing, you do not need the rest of that
larger context to use it.
