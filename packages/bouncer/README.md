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

If you need deterministic time control for tests or simulation work,
drop down to `bouncer-honker`, where the core contract still takes
explicit `now_ms` / `ttl_ms` values.
