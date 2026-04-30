//! Phase 014 pragma-neutrality matrix for core and SQL-extension surfaces.
//!
//! Every row uses a fresh file-backed database so pragma state cannot leak
//! between rows. The matrix proves Bouncer does not set, rewrite, or
//! normalize caller-owned SQLite pragma policy as a side effect of
//! bootstrap or lease operations.
//!
//! The five load-bearing pragmas are:
//! - file-persistent: `journal_mode`, `synchronous`
//! - connection-local: `busy_timeout`, `locking_mode`, `foreign_keys`
//!
//! Use a stable persisted profile (`journal_mode=DELETE`,
//! `synchronous=FULL`) so fresh-connection verification can assert
//! both file-persistent pragmas directly.

use std::path::{Path, PathBuf};

use bouncer_core::{
    attach_bouncer_functions, bootstrap_bouncer_schema, claim, claim_in_tx, ClaimResult,
};
use rusqlite::{params, Connection};
use tempfile::TempDir;

const NAME: &str = "scheduler";
const OWNER: &str = "worker-a";
const NOW_MS: i64 = 1_000;
const TTL_MS: i64 = 500;

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
struct PragmaState {
    journal_mode: String,
    synchronous: i64,
    busy_timeout: i64,
    locking_mode: String,
    foreign_keys: i64,
}

fn set_pragmas(conn: &Connection) -> PragmaState {
    conn.pragma_update(None, "journal_mode", "DELETE").expect("set journal_mode");
    conn.pragma_update(None, "synchronous", "FULL").expect("set synchronous");
    conn.busy_timeout(std::time::Duration::from_millis(777))
        .expect("set busy_timeout");
    conn.pragma_update(None, "locking_mode", "EXCLUSIVE")
        .expect("set locking_mode");
    conn.pragma_update(None, "foreign_keys", "ON").expect("set foreign_keys");

    read_pragmas(conn)
}

fn read_pragmas(conn: &Connection) -> PragmaState {
    let journal_mode: String = conn
        .query_row("PRAGMA journal_mode", [], |row| row.get(0))
        .expect("read journal_mode");
    let synchronous: i64 = conn
        .query_row("PRAGMA synchronous", [], |row| row.get(0))
        .expect("read synchronous");
    let busy_timeout: i64 = conn
        .query_row("PRAGMA busy_timeout", [], |row| row.get(0))
        .expect("read busy_timeout");
    let locking_mode: String = conn
        .query_row("PRAGMA locking_mode", [], |row| row.get(0))
        .expect("read locking_mode");
    let foreign_keys: i64 = conn
        .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
        .expect("read foreign_keys");

    PragmaState {
        journal_mode,
        synchronous,
        busy_timeout,
        locking_mode,
        foreign_keys,
    }
}

fn open_fresh_conn(path: &Path) -> Connection {
    Connection::open(path).expect("open fresh connection")
}

fn assert_same_conn_unchanged(before: &PragmaState, conn: &Connection) {
    let after = read_pragmas(conn);
    assert_eq!(
        before, &after,
        "same-connection pragmas changed after Bouncer operation"
    );
}

fn assert_persistent_unchanged(path: &Path, expected: &PragmaState) {
    let conn = open_fresh_conn(path);
    let after = read_pragmas(&conn);
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
// Core surface
// -----------------------------------------------------------------------

#[test]
fn core_bootstrap_leaves_pragmas_alone() {
    let db = DbFile::fresh();
    let conn = open_fresh_conn(&db.path);

    let before = set_pragmas(&conn);

    bootstrap_bouncer_schema(&conn).expect("bootstrap");

    assert_same_conn_unchanged(&before, &conn);
    drop(conn);
    assert_persistent_unchanged(&db.path, &before);
}

#[test]
fn core_autocommit_claim_leaves_pragmas_alone() {
    let db = DbFile::fresh();
    let conn = open_fresh_conn(&db.path);

    let before = set_pragmas(&conn);
    bootstrap_bouncer_schema(&conn).expect("bootstrap");
    let result = claim(&conn, NAME, OWNER, NOW_MS, TTL_MS).expect("claim");
    assert!(matches!(result, ClaimResult::Acquired(_)));

    assert_same_conn_unchanged(&before, &conn);
    drop(conn);
    assert_persistent_unchanged(&db.path, &before);
}

#[test]
fn core_in_tx_claim_leaves_pragmas_alone() {
    let db = DbFile::fresh();
    let conn = open_fresh_conn(&db.path);

    let before = set_pragmas(&conn);
    bootstrap_bouncer_schema(&conn).expect("bootstrap");
    conn.execute_batch("BEGIN").expect("begin");

    let result = claim_in_tx(&conn, NAME, OWNER, NOW_MS, TTL_MS).expect("claim_in_tx");
    assert!(matches!(result, ClaimResult::Acquired(_)));

    assert_same_conn_unchanged(&before, &conn);
    conn.execute_batch("ROLLBACK").expect("rollback");
    drop(conn);
    assert_persistent_unchanged(&db.path, &before);
}

// -----------------------------------------------------------------------
// SQL extension surface
// -----------------------------------------------------------------------

#[test]
fn sql_bootstrap_leaves_pragmas_alone() {
    let db = DbFile::fresh();
    let conn = open_fresh_conn(&db.path);

    let before = set_pragmas(&conn);
    attach_bouncer_functions(&conn).expect("attach functions");

    let bootstrapped: i64 = conn
        .query_row("SELECT bouncer_bootstrap()", [], |row| row.get(0))
        .expect("bouncer_bootstrap");
    assert_eq!(bootstrapped, 1);

    assert_same_conn_unchanged(&before, &conn);
    drop(conn);
    assert_persistent_unchanged(&db.path, &before);
}

#[test]
fn sql_autocommit_claim_leaves_pragmas_alone() {
    let db = DbFile::fresh();
    let conn = open_fresh_conn(&db.path);

    let before = set_pragmas(&conn);
    attach_bouncer_functions(&conn).expect("attach functions");
    bootstrap_bouncer_schema(&conn).expect("bootstrap");

    let token: Option<i64> = conn
        .query_row(
            "SELECT bouncer_claim(?1, ?2, ?3, ?4)",
            params![NAME, OWNER, TTL_MS, NOW_MS],
            |row| row.get(0),
        )
        .expect("bouncer_claim");
    assert_eq!(token, Some(1));

    assert_same_conn_unchanged(&before, &conn);
    drop(conn);
    assert_persistent_unchanged(&db.path, &before);
}

#[test]
fn sql_savepoint_claim_leaves_pragmas_alone() {
    let db = DbFile::fresh();
    let conn = open_fresh_conn(&db.path);

    let before = set_pragmas(&conn);
    attach_bouncer_functions(&conn).expect("attach functions");
    bootstrap_bouncer_schema(&conn).expect("bootstrap");

    conn.execute_batch("BEGIN").expect("begin outer tx");
    conn.execute_batch("SAVEPOINT lease_ops").expect("savepoint");

    let token: Option<i64> = conn
        .query_row(
            "SELECT bouncer_claim(?1, ?2, ?3, ?4)",
            params![NAME, OWNER, TTL_MS, NOW_MS],
            |row| row.get(0),
        )
        .expect("bouncer_claim in savepoint");
    assert_eq!(token, Some(1));

    assert_same_conn_unchanged(&before, &conn);
    conn.execute_batch("ROLLBACK TO lease_ops").expect("rollback to");
    conn.execute_batch("RELEASE lease_ops").expect("release");
    conn.execute_batch("ROLLBACK").expect("rollback outer");
    drop(conn);
    assert_persistent_unchanged(&db.path, &before);
}
