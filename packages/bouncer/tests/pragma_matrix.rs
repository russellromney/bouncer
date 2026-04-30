//! Phase 014 pragma-neutrality matrix for Rust wrapper surfaces.
//!
//! Every row uses a fresh file-backed database so pragma state cannot leak
//! between rows. The matrix proves the wrapper does not set, rewrite, or
//! normalize caller-owned SQLite pragma policy as a side effect of
//! bootstrap or lease operations.
//!
//! The five load-bearing pragmas are:
//! - file-persistent: `journal_mode`, `synchronous`
//! - connection-local: `busy_timeout`, `locking_mode`, `foreign_keys`
//!
//! `Bouncer` delegates all lease operations to `BouncerRef`, so the
//! borrowed path is the effective implementation layer for autocommit
//! mutators. `Transaction` and `Savepoint` wrap `*_in_tx` helpers proven
//! in the core matrix; here we verify file-persistent pragma survival
//! through the wrapper-owned handle shapes.
//!
//! Use a stable persisted profile (`journal_mode=DELETE`,
//! `synchronous=FULL`) for wrapper-owned rows so fresh-connection
//! verification can assert both file-persistent pragmas directly.

use std::path::{Path, PathBuf};
use std::time::Duration;

use bouncer::{Bouncer, BouncerRef, ClaimResult};
use rusqlite::Connection;
use tempfile::TempDir;

const NAME: &str = "scheduler";
const OWNER: &str = "worker-a";
const TTL: Duration = Duration::from_millis(500);

struct DbFile {
    _tempdir: TempDir,
    path: PathBuf,
}

impl DbFile {
    fn fresh() -> Self {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let path = tempdir.path().join("bouncer.sqlite3");
        Self {
            _tempdir: tempdir,
            path,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PersistentPragmas {
    journal_mode: String,
    synchronous: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AllPragmas {
    journal_mode: String,
    synchronous: i64,
    busy_timeout: i64,
    locking_mode: String,
    foreign_keys: i64,
}

fn set_all_pragmas(conn: &Connection) -> AllPragmas {
    conn.pragma_update(None, "journal_mode", "DELETE").expect("set journal_mode");
    conn.pragma_update(None, "synchronous", "FULL").expect("set synchronous");
    conn.busy_timeout(std::time::Duration::from_millis(777))
        .expect("set busy_timeout");
    conn.pragma_update(None, "locking_mode", "EXCLUSIVE")
        .expect("set locking_mode");
    conn.pragma_update(None, "foreign_keys", "ON").expect("set foreign_keys");

    read_all_pragmas(conn)
}

fn set_persistent_pragmas(path: &Path) -> PersistentPragmas {
    let conn = Connection::open(path).expect("open for persistent pragma setup");
    conn.pragma_update(None, "journal_mode", "DELETE").expect("set journal_mode");
    conn.pragma_update(None, "synchronous", "FULL").expect("set synchronous");
    let out = read_persistent_pragmas(&conn);
    drop(conn);
    out
}

fn read_persistent_pragmas(conn: &Connection) -> PersistentPragmas {
    let journal_mode: String = conn
        .query_row("PRAGMA journal_mode", [], |row| row.get(0))
        .expect("read journal_mode");
    let synchronous: i64 = conn
        .query_row("PRAGMA synchronous", [], |row| row.get(0))
        .expect("read synchronous");
    PersistentPragmas {
        journal_mode,
        synchronous,
    }
}

fn read_all_pragmas(conn: &Connection) -> AllPragmas {
    let persistent = read_persistent_pragmas(conn);
    let busy_timeout: i64 = conn
        .query_row("PRAGMA busy_timeout", [], |row| row.get(0))
        .expect("read busy_timeout");
    let locking_mode: String = conn
        .query_row("PRAGMA locking_mode", [], |row| row.get(0))
        .expect("read locking_mode");
    let foreign_keys: i64 = conn
        .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
        .expect("read foreign_keys");
    AllPragmas {
        journal_mode: persistent.journal_mode,
        synchronous: persistent.synchronous,
        busy_timeout,
        locking_mode,
        foreign_keys,
    }
}

fn assert_all_unchanged(before: &AllPragmas, conn: &Connection) {
    let after = read_all_pragmas(conn);
    assert_eq!(before, &after, "same-connection pragmas changed");
}

fn assert_persistent_unchanged(path: &Path, expected: &PersistentPragmas) {
    let conn = Connection::open(path).expect("open fresh connection for verification");
    let after = read_persistent_pragmas(&conn);
    assert_eq!(
        expected.journal_mode, after.journal_mode,
        "journal_mode changed on fresh connection"
    );
    assert_eq!(
        expected.synchronous, after.synchronous,
        "synchronous changed on fresh connection"
    );
}

// -----------------------------------------------------------------------
// Wrapper surface: Bouncer-owned bootstrap — file-persistent pragmas only
// -----------------------------------------------------------------------

#[test]
fn wrapper_bootstrap_leaves_persistent_pragmas_alone() {
    let db = DbFile::fresh();
    let before = set_persistent_pragmas(&db.path);

    let wrapper = Bouncer::open(&db.path).expect("open wrapper");
    wrapper.bootstrap().expect("bootstrap");

    assert_persistent_unchanged(&db.path, &before);
}

// -----------------------------------------------------------------------
// Wrapper surface: BouncerRef (borrowed path) — all five pragmas
// -----------------------------------------------------------------------

#[test]
fn wrapper_borrowed_claim_leaves_pragmas_alone() {
    let db = DbFile::fresh();
    let conn = Connection::open(&db.path).expect("open raw connection");

    let before = set_all_pragmas(&conn);
    let borrowed = BouncerRef::new(&conn);
    borrowed.bootstrap().expect("bootstrap");

    let result = borrowed.claim(NAME, OWNER, TTL).expect("claim");
    assert!(matches!(result, ClaimResult::Acquired(_)));

    assert_all_unchanged(&before, &conn);
    drop(conn);
    assert_persistent_unchanged(
        &db.path,
        &PersistentPragmas {
            journal_mode: before.journal_mode.clone(),
            synchronous: before.synchronous,
        },
    );
}

// -----------------------------------------------------------------------
// Wrapper surface: Bouncer-owned Transaction / Savepoint
// file-persistent pragmas only (connection-local are fresh-connection defaults)
// -----------------------------------------------------------------------

#[test]
fn wrapper_transaction_claim_leaves_persistent_pragmas_alone() {
    let db = DbFile::fresh();
    let before = set_persistent_pragmas(&db.path);

    let mut wrapper = Bouncer::open(&db.path).expect("open wrapper");
    wrapper.bootstrap().expect("bootstrap");

    let tx = wrapper.transaction().expect("begin transaction");
    let result = tx.claim(NAME, OWNER, TTL).expect("claim");
    assert!(matches!(result, ClaimResult::Acquired(_)));
    tx.commit().expect("commit");

    assert_persistent_unchanged(&db.path, &before);
}

#[test]
fn wrapper_savepoint_claim_leaves_persistent_pragmas_alone() {
    let db = DbFile::fresh();
    let before = set_persistent_pragmas(&db.path);

    let mut wrapper = Bouncer::open(&db.path).expect("open wrapper");
    wrapper.bootstrap().expect("bootstrap");

    let mut tx = wrapper.transaction().expect("begin transaction");
    let sp = tx.savepoint().expect("open savepoint");
    let result = sp.claim(NAME, OWNER, TTL).expect("claim");
    assert!(matches!(result, ClaimResult::Acquired(_)));
    sp.commit().expect("commit savepoint");
    tx.commit().expect("commit transaction");

    assert_persistent_unchanged(&db.path, &before);
}
