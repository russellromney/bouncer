//! Phase 012 SQLite behavior matrix for Rust wrapper surfaces.

use std::path::{Path, PathBuf};
use std::time::Duration;

use litelease::{Litelease, LiteleaseRef, ClaimResult};
use litelease_core as core;
use rusqlite::{params, Connection, ErrorCode};
use tempfile::TempDir;

const NAME: &str = "scheduler";
const OWNER_A: &str = "worker-a";
const OWNER_B: &str = "worker-b";
const TTL_MS: i64 = 500;

struct DbFile {
    _tempdir: TempDir,
    path: PathBuf,
}

impl DbFile {
    fn fresh() -> Self {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let path = tempdir.path().join("litelease.sqlite3");
        Self {
            _tempdir: tempdir,
            path,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Expect {
    Acquired,
    LeaseBusy,
    SqliteBusyOrLocked,
}

#[derive(Clone, Copy)]
enum JournalMode {
    Delete,
    Wal,
}

impl JournalMode {
    fn pragma_value(self) -> &'static str {
        match self {
            JournalMode::Delete => "DELETE",
            JournalMode::Wal => "WAL",
        }
    }
}

fn open_conn(path: &Path, journal_mode: JournalMode, busy_timeout: Duration) -> Connection {
    let conn = Connection::open(path).expect("open sqlite connection");
    conn.busy_timeout(busy_timeout).expect("set busy timeout");
    conn.pragma_update(None, "journal_mode", journal_mode.pragma_value())
        .expect("set journal mode");
    core::bootstrap_litelease_schema(&conn).expect("bootstrap litelease schema");
    conn
}

fn assert_wrapper_claim(result: litelease::Result<ClaimResult>, expect: Expect) {
    match (result, expect) {
        (Ok(ClaimResult::Acquired(_)), Expect::Acquired) => {}
        (Ok(ClaimResult::Busy(_)), Expect::LeaseBusy) => {}
        (Err(err), Expect::SqliteBusyOrLocked) if wrapper_error_is_busy_or_locked(&err) => {}
        (other, expected) => panic!("expected {expected:?}, got {other:?}"),
    }
}

fn wrapper_error_is_busy_or_locked(err: &litelease::Error) -> bool {
    match err {
        litelease::Error::Sqlite(err) => sqlite_error_is_busy_or_locked(err),
        litelease::Error::Core(core::Error::Sqlite(err)) => sqlite_error_is_busy_or_locked(err),
        _ => false,
    }
}

fn sqlite_error_is_busy_or_locked(err: &rusqlite::Error) -> bool {
    match err {
        rusqlite::Error::SqliteFailure(sqlite_err, _) => matches!(
            sqlite_err.code,
            ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked
        ),
        _ => false,
    }
}

fn assert_no_row(conn: &Connection) {
    let row_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM litelease_resources WHERE name = ?1",
            params![NAME],
            |row| row.get(0),
        )
        .expect("count litelease row");
    assert_eq!(row_count, 0);
}

fn assert_raw_row(conn: &Connection, expected_owner: &str, expected_token: i64) {
    let row = conn
        .query_row(
            "SELECT owner, token FROM litelease_resources WHERE name = ?1",
            params![NAME],
            |row| Ok((row.get::<_, Option<String>>(0)?, row.get::<_, i64>(1)?)),
        )
        .expect("read litelease row");

    assert_eq!(row, (Some(expected_owner.to_owned()), expected_token));
}

#[test]
fn wrapper_transaction_claims_writer_intent_up_front() {
    let db = DbFile::fresh();
    let mut wrapper_a = Litelease::open(&db.path).expect("open wrapper_a");
    wrapper_a.bootstrap().expect("bootstrap wrapper_a");
    let contender = open_conn(&db.path, JournalMode::Delete, Duration::from_millis(0));

    let tx_a = wrapper_a.transaction().expect("begin immediate wrapper tx");
    assert_wrapper_claim(
        tx_a.claim(NAME, OWNER_A, Duration::from_millis(TTL_MS as u64)),
        Expect::Acquired,
    );

    let err = contender
        .execute_batch("BEGIN IMMEDIATE")
        .expect_err("contending writer should fail before litelease mutation");
    assert!(
        sqlite_error_is_busy_or_locked(&err),
        "expected busy/locked sqlite tx error, got {err:?}"
    );

    tx_a.rollback().expect("rollback wrapper_a tx");
    assert_no_row(&contender);
}

#[test]
fn litelease_ref_deferred_begin_mirrors_core_lock_upgrade_behavior() {
    let db = DbFile::fresh();
    let conn_a = open_conn(&db.path, JournalMode::Delete, Duration::from_millis(0));
    let conn_b = open_conn(&db.path, JournalMode::Delete, Duration::from_millis(0));
    let borrowed_a = LiteleaseRef::new(&conn_a);
    let borrowed_b = LiteleaseRef::new(&conn_b);

    conn_a.execute_batch("BEGIN").expect("begin conn_a");
    conn_b.execute_batch("BEGIN").expect("begin conn_b");

    assert_wrapper_claim(
        borrowed_a.claim(NAME, OWNER_A, Duration::from_millis(TTL_MS as u64)),
        Expect::Acquired,
    );
    assert_wrapper_claim(
        borrowed_b.claim(NAME, OWNER_B, Duration::from_millis(TTL_MS as u64)),
        Expect::SqliteBusyOrLocked,
    );

    // Roll back the blocked deferred reader first; reversing this can make
    // the writer rollback wait on the still-open reader in some journal modes.
    conn_b.execute_batch("ROLLBACK").expect("rollback conn_b");
    conn_a.execute_batch("ROLLBACK").expect("rollback conn_a");
    assert_no_row(&conn_a);
}

#[test]
fn litelease_ref_autocommit_live_lease_returns_lease_busy_in_wal_journal() {
    let db = DbFile::fresh();
    let conn = open_conn(&db.path, JournalMode::Wal, Duration::from_millis(0));
    let borrowed = LiteleaseRef::new(&conn);

    assert_wrapper_claim(
        borrowed.claim(NAME, OWNER_A, Duration::from_millis(TTL_MS as u64)),
        Expect::Acquired,
    );
    assert_wrapper_claim(
        borrowed.claim(NAME, OWNER_B, Duration::from_millis(TTL_MS as u64)),
        Expect::LeaseBusy,
    );

    assert_raw_row(&conn, OWNER_A, 1);
}

#[test]
fn litelease_ref_savepoint_participates_in_caller_owned_boundary() {
    let db = DbFile::fresh();
    let conn = open_conn(&db.path, JournalMode::Delete, Duration::from_millis(0));
    let borrowed = LiteleaseRef::new(&conn);

    conn.execute_batch("BEGIN")
        .expect("begin outer transaction");
    conn.execute_batch("SAVEPOINT lease_ops")
        .expect("open savepoint");
    assert_wrapper_claim(
        borrowed.claim(NAME, OWNER_A, Duration::from_millis(TTL_MS as u64)),
        Expect::Acquired,
    );

    conn.execute_batch("ROLLBACK TO lease_ops")
        .expect("rollback savepoint");
    conn.execute_batch("RELEASE lease_ops")
        .expect("release savepoint");
    conn.execute_batch("COMMIT")
        .expect("commit outer transaction");

    assert_no_row(&conn);
}

#[test]
fn wrapper_typed_savepoint_commit_participates_in_transaction_boundary() {
    let db = DbFile::fresh();
    let mut wrapper = Litelease::open(&db.path).expect("open wrapper");
    wrapper.bootstrap().expect("bootstrap wrapper");

    let mut tx = wrapper.transaction().expect("begin wrapper transaction");
    let sp = tx.savepoint().expect("open wrapper savepoint");
    assert_wrapper_claim(
        sp.claim(NAME, OWNER_A, Duration::from_millis(TTL_MS as u64)),
        Expect::Acquired,
    );
    sp.commit().expect("commit wrapper savepoint");
    tx.commit().expect("commit wrapper transaction");

    let observer = open_conn(&db.path, JournalMode::Delete, Duration::from_millis(0));
    assert_raw_row(&observer, OWNER_A, 1);
}
