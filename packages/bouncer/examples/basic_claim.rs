use std::time::Duration;

use bouncer::{Bouncer, ClaimResult};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db = Bouncer::open("app.sqlite3")?;
    db.bootstrap()?;

    match db.claim("scheduler", "worker-a", Duration::from_secs(30))? {
        ClaimResult::Acquired(lease) => {
            println!("acquired {} with token {}", lease.name, lease.token);
        }
        ClaimResult::Busy(current) => {
            println!("busy: currently owned by {}", current.owner);
        }
    }

    Ok(())
}
