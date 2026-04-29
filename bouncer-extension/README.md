# bouncer-extension

SQLite loadable extension for Bouncer.

Builds `libbouncer_ext.dylib` / `.so` for your platform and exposes the
first `bouncer_*` SQL helpers:

- `bouncer_bootstrap()`
- `bouncer_claim(name, owner, ttl_ms, now_ms)`
- `bouncer_renew(name, owner, ttl_ms, now_ms)`
- `bouncer_release(name, owner, now_ms)`
- `bouncer_owner(name, now_ms)`
- `bouncer_token(name)`

The SQL surface stays explicit about time. It reuses `bouncer-core`
for schema and lease semantics rather than reimplementing them in SQL.

## Loading the extension

SQLite CLI:

```sql
.load /path/to/libbouncer_ext
SELECT bouncer_bootstrap();
```

rusqlite:

```rust
use rusqlite::{Connection, LoadExtensionGuard};

let conn = Connection::open("app.sqlite3")?;
let _guard = unsafe { LoadExtensionGuard::new(&conn)? };
unsafe { conn.load_extension("/path/to/libbouncer_ext", None::<&str>)?; }
conn.query_row("SELECT bouncer_bootstrap()", [], |_| Ok(()))?;
```

Raw SQLite C API / SQL:

- enable extension loading on the connection
- load `libbouncer_ext.{dylib,so,dll}`
- call `SELECT bouncer_bootstrap()`

## Transaction model

`bouncer_claim`, `bouncer_renew`, and `bouncer_release` work in both:

- normal autocommit mode
- an already-open explicit transaction or savepoint on the caller's
  connection

In autocommit mode they delegate to the same shared Rust helpers as
direct callers, which open `BEGIN IMMEDIATE`.

That means this works:

```sql
SELECT bouncer_claim('scheduler', 'worker-a', 5000, 100);
```

And this now works too:

```sql
BEGIN IMMEDIATE;
SELECT bouncer_claim('scheduler', 'worker-a', 5000, 100);
COMMIT;
```

Inside an already-open transaction, Bouncer reuses the caller's current
transaction state instead of attempting a nested `BEGIN IMMEDIATE`.

One important caveat: lock-upgrade timing in that path follows the
caller’s outer transaction mode. `BEGIN IMMEDIATE` gives you the same
up-front writer claim as the direct Rust path. Plain `BEGIN` can still
surface SQLite lock/busy behavior before Bouncer gets to finish the
lease-level decision, and the user-visible behavior will also depend on
the connection's `busy_timeout`.
