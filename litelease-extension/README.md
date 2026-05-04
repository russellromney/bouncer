# litelease-extension

SQLite loadable extension for Litelease.

This is the connection-owned surface.

Use it when your app already has a SQLite connection and you want
Litelease to participate on that exact connection instead of opening a new
wrapper-owned one.

Builds `liblitelease_ext.dylib` / `.so` for your platform and exposes the
first `litelease_*` SQL helpers:

- `litelease_bootstrap()`
- `litelease_claim(name, owner, ttl_ms, now_ms)`
- `litelease_renew(name, owner, ttl_ms, now_ms)`
- `litelease_release(name, owner, now_ms)`
- `litelease_owner(name, now_ms)`
- `litelease_token(name)`

The SQL surface stays explicit about time. It reuses `litelease-core`
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

- macOS: `target/release/liblitelease_ext.dylib`
- Linux: `target/release/liblitelease_ext.so`
- Windows: `target/release/litelease_ext.dll`

If you want a release-shaped asset staged locally with a checksum file:

```bash
make dist-ext
```

That stages a current-platform file in `dist/` with a stable release
name like `litelease-extension-macos-arm64.dylib` plus a matching
`.sha256`.

The repo also includes a GitHub Actions workflow that builds these
artifacts for tagged releases, renames them into those stable
platform-specific asset names, and uploads both the asset and its
checksum file.

## Loading the extension

SQLite CLI:

```sql
.load /path/to/liblitelease_ext
SELECT litelease_bootstrap();
```

rusqlite:

```rust
use rusqlite::{Connection, LoadExtensionGuard};

let conn = Connection::open("app.sqlite3")?;
let _guard = unsafe { LoadExtensionGuard::new(&conn)? };
unsafe { conn.load_extension("/path/to/liblitelease_ext", None::<&str>)?; }
conn.query_row("SELECT litelease_bootstrap()", [], |_| Ok(()))?;
```

Raw SQLite C API / SQL:

- enable extension loading on the connection
- load `liblitelease_ext.{dylib,so,dll}`
- call `SELECT litelease_bootstrap()`

Python `sqlite3`:

```python
import sqlite3

conn = sqlite3.connect("app.sqlite3")
conn.enable_load_extension(True)
# Load the exact asset you built or downloaded.
# Example: dist/litelease-extension-macos-arm64.dylib
conn.load_extension("dist/litelease-extension-macos-arm64.dylib")
conn.execute("SELECT litelease_bootstrap()")
```

See also:

- [examples/basic_claim.py](/Users/russellromney/Documents/Github/bouncer/litelease-extension/examples/basic_claim.py)
- [examples/transactional_claim.py](/Users/russellromney/Documents/Github/bouncer/litelease-extension/examples/transactional_claim.py)

## Smoke proof

From the repo root:

```bash
make smoke-ext
```

That builds the release artifact and runs a real load-and-call smoke
path against it through a public caller boundary.

## Transaction model

`litelease_claim`, `litelease_renew`, and `litelease_release` work in both:

- normal autocommit mode
- an already-open explicit transaction or savepoint on the caller's
  connection

In autocommit mode they delegate to the same shared Rust helpers as
direct callers, which open `BEGIN IMMEDIATE`.

That means this works:

```sql
SELECT litelease_claim('scheduler', 'worker-a', 5000, 100);
```

And this now works too:

```sql
BEGIN IMMEDIATE;
SELECT litelease_claim('scheduler', 'worker-a', 5000, 100);
COMMIT;
```

Inside an already-open transaction, Litelease reuses the caller's current
transaction state instead of attempting a nested `BEGIN IMMEDIATE`.

One important caveat: lock-upgrade timing in that path follows the
caller’s outer transaction mode. `BEGIN IMMEDIATE` gives you the same
up-front writer claim as the direct Rust path. Plain `BEGIN` can still
surface SQLite lock/busy behavior before Litelease gets to finish the
lease-level decision, and the user-visible behavior will also depend on
the connection's `busy_timeout`.
