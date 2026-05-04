# bouncer-extension

SQLite loadable extension for Bouncer.

This is the connection-owned surface.

Use it when your app already has a SQLite connection and you want
Bouncer to participate on that exact connection instead of opening a new
wrapper-owned one.

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

This makes the extension the right boundary for:

- raw SQLite clients
- apps that already own a connection in Python, Go, Ruby, etc.
- cross-language interop on one database file
- business writes plus lease mutations in one caller-owned transaction

## Build and release

From the repo root:

```bash
make build-ext
```

That produces:

- macOS: `target/release/libbouncer_ext.dylib`
- Linux: `target/release/libbouncer_ext.so`
- Windows: `target/release/bouncer_ext.dll`

If you want a release-shaped asset staged locally with a checksum file:

```bash
make dist-ext
```

That stages a current-platform file in `dist/` with a stable release
name like `bouncer-extension-macos-arm64.dylib` plus a matching
`.sha256`.

The repo also includes a GitHub Actions workflow that builds these
artifacts for tagged releases, renames them into those stable
platform-specific asset names, and uploads both the asset and its
checksum file.

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

Python `sqlite3`:

```python
import sqlite3

conn = sqlite3.connect("app.sqlite3")
conn.enable_load_extension(True)
# Load the exact asset you built or downloaded.
# Example: dist/bouncer-extension-macos-arm64.dylib
conn.load_extension("dist/bouncer-extension-macos-arm64.dylib")
conn.execute("SELECT bouncer_bootstrap()")
```

See also:

- [examples/basic_claim.py](/Users/russellromney/Documents/Github/bouncer/bouncer-extension/examples/basic_claim.py)
- [examples/transactional_claim.py](/Users/russellromney/Documents/Github/bouncer/bouncer-extension/examples/transactional_claim.py)

## Smoke proof

From the repo root:

```bash
make smoke-ext
```

That builds the release artifact and runs a real load-and-call smoke
path against it through a public caller boundary.

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
