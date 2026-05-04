# litelease crate

Rust wrapper for Litelease.

Litelease is a SQLite lease primitive with expiry and fencing tokens. This
crate is the Rust convenience layer on top of that shared lease state
machine.

Use this crate when:

- you are already in Rust
- you want typed results
- you want a sanctioned transaction/savepoint surface

Do not use this crate to invent a second SQLite story. If you already
own the `rusqlite::Connection`, use `LiteleaseRef` on that connection so
the lease mutation participates in the same SQLite boundary as the rest
of your work.

## Surfaces

- `Litelease`
  wrapper-owned connection, easiest default for most Rust callers
- `Litelease::transaction()`
  sanctioned `BEGIN IMMEDIATE` path for business writes plus lease
  mutations in one atomic boundary
- `LiteleaseRef`
  caller-owned `rusqlite::Connection`, transaction, or savepoint

The wrapper does not:

- reimplement lease semantics
- hide pragma policy
- use wall clock as an ordering primitive

Wall clock in this crate is only for expiry bookkeeping. The underlying
lease contract still comes from `litelease-core`.

## Example

```rust
use std::time::Duration;

use litelease::{Litelease, ClaimResult};

let db = Litelease::open("app.sqlite3")?;
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

## Atomic business write + lease mutation

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

`tx.conn()` gives you the underlying `rusqlite::Connection` for
business writes inside the same atomic boundary. Use the handle's
`commit()` / `rollback()` to finish the transaction.

If you need a nested rollback boundary, use `tx.savepoint()`.

## Notes

- Call `bootstrap()` explicitly. `open(path)` does not create schema.
- Configure your SQLite connection policy yourself. The wrapper is
  pragma-neutral.
- If you need explicit `now_ms` control for tests or simulation, drop
  down to `litelease-core`.
