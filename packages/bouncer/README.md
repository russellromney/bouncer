# bouncer package

Thin Rust binding for `bouncer-honker`.

This crate is the friendly wrapper layer:

- `bouncer-honker` keeps the explicit-time core contract
- `bouncer` opens a SQLite database, bootstraps the schema explicitly,
  and exposes the four lease verbs with normal `Duration` inputs

It does not:

- reimplement lease semantics
- invent a second state machine
- hide SQLite connection policy
- use wall clock as an ordering primitive

If the caller already owns a transaction or savepoint on a borrowed
`rusqlite::Connection`, `BouncerRef` mutators participate in that
existing boundary instead of attempting a nested transaction.

Example:

```rust
use std::time::Duration;

use bouncer::{Bouncer, ClaimResult};

let db = Bouncer::open("app.sqlite3")?;
db.bootstrap()?;

match db.claim("scheduler", "worker-a", Duration::from_secs(30))? {
    ClaimResult::Acquired(lease) => {
        println!("got lease token {}", lease.token);
    }
    ClaimResult::Busy(current) => {
        println!("currently owned by {}", current.owner);
    }
}
# Ok::<(), Box<dyn std::error::Error>>(())
```

If multiple processes or connections will touch the same database file,
configure those connections first, then call `bootstrap()`. In practice
that usually means `journal_mode=WAL` plus a non-zero `busy_timeout`.
The wrapper stays pragma-neutral on purpose.

To combine a business write and a lease mutation atomically, open a
transaction handle. `wrapper.transaction()` takes `&mut self`, so the
borrow checker prevents a second open transaction or a stray
autocommit call on the same `Bouncer` while one is alive.

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

`tx.conn()` returns the underlying `rusqlite::Connection` for prepared
statements and business writes inside the same atomic boundary. Don't
issue `BEGIN` / `COMMIT` / `ROLLBACK` through it — call the handle's
`commit()` / `rollback()` (or drop the handle to roll back).

If you need deterministic time control for tests or simulation work,
drop down to `bouncer-honker`, where the core contract still takes
explicit `now_ms` / `ttl_ms` values.
