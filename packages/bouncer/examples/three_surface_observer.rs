//! Standalone three-surface interop observer.
//!
//! Prints a single line of JSON for how the Rust wrapper surface sees a
//! lease on a given file.  Used by the Python acceptance suite to
//! prove that wrapper, SQL extension, and Python binding all observe
//! the same database-state snapshot.
//!
//! Usage:
//!   cargo run --example three_surface_observer -- <db_path> <name>
//!
//! Expected stdout:
//!   {"exists":true,"owner":"worker-a","token":3}
//! or
//!   {"exists":false}

use std::env;

use bouncer::Bouncer;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        eprintln!("Usage: {} <db_path> <name>", args[0]);
        std::process::exit(1);
    }

    let db = Bouncer::open(&args[1]).expect("open db");
    match db.inspect(&args[2]).expect("inspect") {
        Some(lease) => println!(
            "{{\"exists\":true,\"owner\":\"{}\",\"token\":{}}}",
            lease.owner, lease.token
        ),
        None => println!("{{\"exists\":false}}"),
    }
}
