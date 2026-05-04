use std::time::Duration;

use litelease::{Bouncer, ClaimResult};
use rusqlite::params;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut db = Bouncer::open("app.sqlite3")?;
    db.bootstrap()?;

    let tx = db.transaction()?;
    tx.conn().execute(
        "CREATE TABLE IF NOT EXISTS jobs (payload TEXT NOT NULL)",
        [],
    )?;
    tx.conn().execute(
        "INSERT INTO jobs(payload) VALUES (?1)",
        params!["run scheduler tick"],
    )?;

    match tx.claim("scheduler", "worker-a", Duration::from_secs(30))? {
        ClaimResult::Acquired(lease) => {
            println!("acquired token {}; committing", lease.token);
            tx.commit()?;
        }
        ClaimResult::Busy(current) => {
            println!("busy: owned by {}; rolling back", current.owner);
            tx.rollback()?;
        }
    }

    Ok(())
}
