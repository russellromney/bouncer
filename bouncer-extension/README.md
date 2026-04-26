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

The SQL surface stays explicit about time. It reuses `bouncer-honker`
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

`bouncer_claim`, `bouncer_renew`, and `bouncer_release` are autocommit-mode
helpers. Internally they open `BEGIN IMMEDIATE` transactions through the
shared Rust core.

That means this works:

```sql
SELECT bouncer_claim('scheduler', 'worker-a', 5000, 100);
```

But this does not:

```sql
BEGIN;
SELECT bouncer_claim('scheduler', 'worker-a', 5000, 100);
COMMIT;
```

Inside an already-open explicit transaction, SQLite will return its
nested-transaction error instead of weakening the locking model.
