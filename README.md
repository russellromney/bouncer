# litelease

Leases, ownership, and fencing tokens for SQLite apps.

## What is this?

Litelease answers one small question: who owns this named resource right
now?

It gives a normal SQLite application a durable lease primitive in the
same database file it already uses. That makes it useful for things
like one scheduler running at a time, one background worker owning a
shard, one importer holding exclusive ownership while it works, or one
process acting as leader for some local responsibility.

Litelease is not a queue, a workflow engine, or distributed consensus. It
is a small SQLite state machine with expiry and fencing tokens.

## Status

Litelease is real and heavily tested, but still early. Today it has a
Rust core, a SQLite loadable extension, a Rust wrapper, and direct proof
for semantics, SQLite behavior, schema hardening, pragma-neutrality, and
user-shaped acceptance.

What it does not have yet is a migration story, a package-manager-first
install story across every surface, or any claim to distributed
consensus or multi-machine coordination.

The clearest way to think about Litelease today is: the SQL extension is the
base interoperability surface, and the Rust wrapper is the main typed
convenience layer.

## How do I try it?

From the repo root:

```bash
make test-rust
make build-ext
make smoke-ext
```

If you want the quickest hands-on path, start with:

- [litelease-extension/examples/basic_claim.sql](/Users/russellromney/Documents/Github/bouncer/litelease-extension/examples/basic_claim.sql)
- [litelease-extension/examples/basic_claim.py](/Users/russellromney/Documents/Github/bouncer/litelease-extension/examples/basic_claim.py)
- [packages/litelease/examples/basic_claim.rs](/Users/russellromney/Documents/Github/bouncer/packages/litelease/examples/basic_claim.rs)

## How do I install it?

Today the cleanest way to use Litelease is still from source.

- **SQL extension**: build `litelease-extension` and load the resulting
  `liblitelease_ext.{dylib,so,dll}` into the SQLite connection you already
  own
- **Rust wrapper**: use the `litelease` crate from this repo today
- **Python**: use stdlib `sqlite3` and load the extension on the
  connection you already manage

From the repo root, the extension build is:

```bash
make build-ext
```

That builds the local artifact under `target/release/`.

If you want a release-shaped file plus checksum staged locally:

```bash
make dist-ext
```

That produces a current-platform asset in `dist/` with a stable name
like:

- `litelease-extension-linux-x86_64.so`
- `litelease-extension-macos-arm64.dylib`
- `litelease-extension-windows-x86_64.dll`

and a matching `.sha256` file beside it.

Tagged GitHub releases also attach those same platform-specific assets
plus their checksum files.

## Why does it exist?

Plenty of apps already have the hard part they need: one SQLite file
shared by a few threads, processes, jobs, or local daemons.

What they usually do not have is a clean answer to:

- "who owns this job runner right now?"
- "how do I fail over if the owner dies?"
- "how do I stop a stale actor from continuing work after takeover?"

The usual alternatives are all annoying in different ways: ad hoc lock
tables with fuzzy semantics, PID files and temp files, hand-rolled
"heartbeat" rows, or dragging in Redis or a bigger coordination system
for a local app.

Litelease exists to make that boring.

## Why not just a lock table?

A lock table can be enough for a rough internal tool. Litelease exists for
the parts that usually stay under-specified: what reclaim means after
expiry, how stale actors are fenced off, how lease conflict differs from
SQLite `BUSY` / `LOCKED`, how the lease mutation should participate in
caller-owned transactions and savepoints, and what happens when the
persisted state is invalid or has drifted from the expected schema.

The happy path can be a handful of SQL statements. The edge-case
contract is the larger part of the problem.

## Why would I use it?

Use Litelease when you already use SQLite, want one durable owner of a
named resource, want expiry and takeover to be explicit, care about
stale-actor protection, and want to keep coordination in the same file
as the rest of the app.

Do not use Litelease when you need cross-machine consensus, fairness or
waiting queues, a job system rather than a lease primitive, or Litelease
itself to enforce downstream stale-write rejection.

## Limitations

Litelease is intentionally narrow. It is for a single machine and one
SQLite file. It does not have a migration story today, it does not try
to provide fairness or waiting-queue semantics, and it is not
distributed consensus. Downstream token enforcement is still the
caller’s job.

## Design at a glance

Litelease stores lease state in one table, `litelease_resources`. Each
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
| Rust `Litelease` | wrapper-owned connection | normal Rust apps that want typed results |
| Rust `LiteleaseRef` | caller | Rust code that already owns a `rusqlite::Connection` or transaction |

The shortest rule is:

- if **you** already own the SQLite connection, use the **SQL extension**
- if you are in **Rust** and want a wrapper-owned path, use **`Litelease`**
- if you are in **Rust** and already own the connection, use **`LiteleaseRef`**
- if you are in **Python**, use stdlib `sqlite3` plus the **SQL extension**

### SQL extension

Use this when you already own the SQLite connection and want Litelease to
participate on that exact connection.

```sql
SELECT litelease_bootstrap();
SELECT litelease_claim('scheduler', 'worker-a', 30000, 1700000000000);
SELECT litelease_owner('scheduler', 1700000000000);
SELECT litelease_token('scheduler');
```

Tiny transaction example:

```sql
BEGIN IMMEDIATE;
INSERT INTO jobs(payload) VALUES ('work');
SELECT litelease_claim('scheduler', 'worker-a', 30000, 1700000000000);
COMMIT;
```

The SQL surface keeps time explicit. You pass `now_ms` yourself. The SQL
prefix is still `litelease_*` today.

See also:

- [litelease-extension/examples/basic_claim.sql](/Users/russellromney/Documents/Github/bouncer/litelease-extension/examples/basic_claim.sql)
- [litelease-extension/examples/transactional_claim.sql](/Users/russellromney/Documents/Github/bouncer/litelease-extension/examples/transactional_claim.sql)
- [litelease-extension/examples/basic_claim.py](/Users/russellromney/Documents/Github/bouncer/litelease-extension/examples/basic_claim.py)
- [litelease-extension/examples/transactional_claim.py](/Users/russellromney/Documents/Github/bouncer/litelease-extension/examples/transactional_claim.py)

### Rust wrapper

Use this when you are writing Rust and want typed results plus a
sanctioned transaction/savepoint surface.

```rust
use std::time::Duration;

use litelease::{Litelease, ClaimResult};

let db = Litelease::open("app.sqlite3")?;
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

- [packages/litelease/examples/basic_claim.rs](/Users/russellromney/Documents/Github/bouncer/packages/litelease/examples/basic_claim.rs)
- [packages/litelease/examples/transactional_claim.rs](/Users/russellromney/Documents/Github/bouncer/packages/litelease/examples/transactional_claim.rs)

The published Rust crate is `litelease`. `Litelease` is still the main
wrapper type, and `LiteleaseRef` is the lower-level
integration surface when your code already owns the `rusqlite::Connection`
or the current transaction/savepoint boundary.

### Python

Python is not a separate package surface anymore. The intended Python
story is stdlib `sqlite3` plus the SQL extension on the connection you
already own.

```python
import sqlite3

conn = sqlite3.connect("app.sqlite3")
conn.enable_load_extension(True)
# Load the exact asset you built or downloaded.
# Example: dist/litelease-extension-macos-arm64.dylib
conn.load_extension("dist/litelease-extension-macos-arm64.dylib")

conn.execute("SELECT litelease_bootstrap()")
token = conn.execute(
    "SELECT litelease_claim(?, ?, ?, ?)",
    ("scheduler", "worker-a", 30_000, 1_700_000_000_000),
).fetchone()[0]

print(token)
```

If you are already in Python, this keeps the boundary simple: one
SQLite connection, one extension, one set of SQL functions.

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

SQL transaction example:

```sql
BEGIN IMMEDIATE;
INSERT INTO jobs(payload) VALUES ('work');
SELECT litelease_claim('scheduler', 'worker-a', 30000, 1700000000000);
COMMIT;
```

Rust wrapper example:

```rust
use std::time::Duration;

use litelease::{Litelease, ClaimResult};
use rusqlite::params;

let mut db = Litelease::open("app.sqlite3")?;
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

Python `sqlite3` example:

```python
conn.execute("BEGIN IMMEDIATE")
conn.execute("INSERT INTO jobs(payload) VALUES (?)", ("work",))
token = conn.execute(
    "SELECT litelease_claim(?, ?, ?, ?)",
    ("scheduler", "worker-a", 30_000, 1_700_000_000_000),
).fetchone()[0]
conn.commit()
```

## How does the transaction story work?

There are two modes. In autocommit, Litelease owns the transaction and
opens `BEGIN IMMEDIATE`. Inside a caller-owned transaction or savepoint,
Litelease reuses the boundary you already opened. That means it does not
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

If another owner already holds a live lease, Litelease returns a lease
busy result: Rust gives `ClaimResult::Busy`, and SQL returns `NULL`
from `litelease_claim(...)`. That is different from SQLite `BUSY` /
`LOCKED`, which means writer contention or deferred lock-upgrade
failure.

### Litelease is pragma-neutral

Litelease does not set or normalize your connection policy. The current
proved set is `journal_mode`, `synchronous`, `busy_timeout`,
`locking_mode`, and `foreign_keys`. Set them yourself before calling
`bootstrap()` or before handing a connection to `LiteleaseRef`.

### Bootstrap is strict

If `litelease_resources` already exists with the wrong shape, bootstrap
fails loudly with `SchemaMismatch`. Litelease is not a migration engine
and does not silently accept drifted schema.

### Fencing tokens matter only if you carry them

Litelease guarantees monotonic tokens. It cannot make your downstream
systems check them for you. If stale-actor protection matters beyond
SQLite, carry the token to the place where side effects happen.

## FAQ

### Why not just use a lock table?

You can, if the happy path is all you need. Litelease exists for the
harder parts: expiry/reclaim semantics, monotonic fencing tokens, lease
busy versus SQLite busy/locked, transaction/savepoint participation, and
strict handling of drifted or invalid persisted state.

### Why not just use Redis or Postgres advisory locks?

If your app already runs Redis or Postgres and that solves the right
problem for you, that can be a fine choice. Litelease is for the narrower
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
wrapper-owned connection. Use `LiteleaseRef` when Rust already owns the
connection and you want Litelease to participate in that exact SQLite
boundary.

### What should Python users do?

Use stdlib `sqlite3`, enable extension loading, and load
`liblitelease_ext`. That keeps Python on the same base surface as every
other caller-owned SQLite integration instead of adding a second Python
API.

## Current proof

The repo now has direct proof for core lease semantics, deterministic
invariant coverage, SQLite busy/locked versus lease-busy behavior,
strict schema-drift rejection and invalid-row hardening,
pragma-neutrality across the main public surfaces, narrative
user-shaped acceptance journeys, and a heavier repeated public-surface
stress layer on one real database file.

That acceptance layer includes fresh bootstrap plus first claim,
independent second-caller busy, release/reclaim token increase,
deterministic expiry/reclaim token increase, transaction atomic
visibility, loud drifted-schema bootstrap failure, direct Rust
wrapper / SQL extension cross-surface visibility, and repeated wrapper
/ SQL stress rows that hammer claim, busy, renew, release, reclaim,
expiry handoff, and caller-owned transaction participation.

## Repository map

- [litelease-core](/Users/russellromney/Documents/Github/bouncer/litelease-core)
  Rust core owning schema and lease semantics
- [litelease-extension](/Users/russellromney/Documents/Github/bouncer/litelease-extension)
  SQLite loadable extension
- [packages/litelease](/Users/russellromney/Documents/Github/bouncer/packages/litelease)
  Rust wrapper

## Development

```bash
make test-rust
make smoke-ext
make test
```

## License

Dual-licensed under either:

- [MIT](/Users/russellromney/Documents/Github/bouncer/LICENSE-MIT)
- [Apache-2.0](/Users/russellromney/Documents/Github/bouncer/LICENSE-APACHE)

at your option.

## Origins

Litelease started alongside other SQLite infrastructure work, but the
primitive stands on its own. If you have a single-machine SQLite app and
need durable ownership with fencing, you do not need the rest of that
larger context to use it.
