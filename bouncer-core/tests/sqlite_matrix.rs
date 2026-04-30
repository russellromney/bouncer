//! Phase 012 SQLite behavior matrix for core and SQL-extension surfaces.
//!
//! Every row uses a fresh file-backed database so lock and journal-mode state
//! cannot leak between rows.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use bouncer_core::{
    attach_bouncer_functions, bootstrap_bouncer_schema, claim, claim_in_tx, inspect, release_in_tx,
    renew_in_tx, ClaimResult, LeaseInfo,
};
use rusqlite::{params, Connection, ErrorCode, OptionalExtension};
use tempfile::TempDir;

const NAME: &str = "scheduler";
const OWNER_A: &str = "worker-a";
const OWNER_B: &str = "worker-b";
const NOW_MS: i64 = 1_000;
const TTL_MS: i64 = 500;

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

#[derive(Debug, PartialEq, Eq)]
enum Expect {
    Acquired,
    LeaseBusy,
    SqliteBusyOrLocked,
    SqliteUdfUnknownWithBusyText,
}

fn open_core_conn(path: &Path, journal_mode: JournalMode, busy_timeout: Duration) -> Connection {
    let conn = Connection::open(path).expect("open core sqlite connection");
    configure_conn(&conn, journal_mode, busy_timeout);
    bootstrap_bouncer_schema(&conn).expect("bootstrap bouncer schema");
    conn
}

fn open_sql_conn(path: &Path, journal_mode: JournalMode, busy_timeout: Duration) -> Connection {
    let conn = Connection::open(path).expect("open sql sqlite connection");
    configure_conn(&conn, journal_mode, busy_timeout);
    attach_bouncer_functions(&conn).expect("attach bouncer sql functions");
    bootstrap_bouncer_schema(&conn).expect("bootstrap bouncer schema");
    conn
}

fn configure_conn(conn: &Connection, journal_mode: JournalMode, busy_timeout: Duration) {
    conn.busy_timeout(busy_timeout).expect("set busy timeout");
    conn.pragma_update(None, "journal_mode", journal_mode.pragma_value())
        .expect("set journal mode");
}

fn assert_core_claim(result: bouncer_core::Result<ClaimResult>, expect: Expect) {
    match (result, expect) {
        (Ok(ClaimResult::Acquired(_)), Expect::Acquired) => {}
        (Ok(ClaimResult::Busy(_)), Expect::LeaseBusy) => {}
        (Err(err), Expect::SqliteBusyOrLocked) if core_error_is_busy_or_locked(&err) => {}
        (other, expected) => panic!("expected {expected:?}, got {other:?}"),
    }
}

fn assert_core_lock_failure<T: std::fmt::Debug>(result: bouncer_core::Result<T>) {
    match result {
        Err(err) if core_error_is_busy_or_locked(&err) => {}
        other => panic!("expected SQLite busy/locked failure, got {other:?}"),
    }
}

fn assert_sql_claim(result: rusqlite::Result<Option<i64>>, expect: Expect) {
    match (result, expect) {
        (Ok(Some(_)), Expect::Acquired) => {}
        (Ok(None), Expect::LeaseBusy) => {}
        (Err(err), Expect::SqliteBusyOrLocked) if sqlite_error_is_busy_or_locked(&err) => {}
        (Err(err), Expect::SqliteUdfUnknownWithBusyText)
            if sqlite_udf_unknown_error_message_is_busy_or_locked(&err) => {}
        (other, expected) => panic!("expected {expected:?}, got {other:?}"),
    }
}

fn core_error_is_busy_or_locked(err: &bouncer_core::Error) -> bool {
    match err {
        bouncer_core::Error::Sqlite(err) => sqlite_error_is_busy_or_locked(err),
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

fn sqlite_udf_unknown_error_message_is_busy_or_locked(err: &rusqlite::Error) -> bool {
    let rusqlite::Error::SqliteFailure(sqlite_err, Some(message)) = err else {
        return false;
    };

    // SQLite/rusqlite scalar-function callbacks can collapse a returned
    // BUSY/LOCKED SQLite error to SQLITE_ERROR while preserving only SQLite's
    // busy/locked message. This fallback is intentionally isolated to rows
    // that expect that UDF boundary behavior.
    sqlite_err.code == ErrorCode::Unknown
        && (message.contains("database is busy") || message.contains("database is locked"))
}

fn sql_claim(
    conn: &Connection,
    name: &str,
    owner: &str,
    now_ms: i64,
    ttl_ms: i64,
) -> rusqlite::Result<Option<i64>> {
    conn.query_row(
        "SELECT bouncer_claim(?1, ?2, ?3, ?4)",
        params![name, owner, ttl_ms, now_ms],
        |row| row.get(0),
    )
}

fn assert_live_lease(
    conn: &Connection,
    now_ms: i64,
    owner: &str,
    token: i64,
    lease_expires_at_ms: i64,
) {
    let lease = inspect(conn, NAME, now_ms)
        .expect("inspect lease")
        .expect("live lease");
    assert_eq!(
        lease,
        LeaseInfo {
            name: NAME.to_owned(),
            owner: owner.to_owned(),
            token,
            lease_expires_at_ms,
        }
    );
}

fn assert_no_row(conn: &Connection) {
    let row_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM bouncer_resources WHERE name = ?1",
            params![NAME],
            |row| row.get(0),
        )
        .expect("count bouncer row");
    assert_eq!(row_count, 0);
}

fn assert_raw_row(conn: &Connection, expected_owner: &str, expected_token: i64) {
    let row = conn
        .query_row(
            "SELECT owner, token FROM bouncer_resources WHERE name = ?1",
            params![NAME],
            |row| Ok((row.get::<_, Option<String>>(0)?, row.get::<_, i64>(1)?)),
        )
        .optional()
        .expect("read bouncer row")
        .expect("bouncer row");

    assert_eq!(row, (Some(expected_owner.to_owned()), expected_token));
}

#[test]
fn core_autocommit_live_lease_returns_lease_busy_in_delete_journal() {
    let db = DbFile::fresh();
    let conn = open_core_conn(&db.path, JournalMode::Delete, Duration::from_millis(0));

    assert_core_claim(
        claim(&conn, NAME, OWNER_A, NOW_MS, TTL_MS),
        Expect::Acquired,
    );
    assert_core_claim(
        claim(&conn, NAME, OWNER_B, NOW_MS + 1, TTL_MS),
        Expect::LeaseBusy,
    );

    assert_live_lease(&conn, NOW_MS + 1, OWNER_A, 1, NOW_MS + TTL_MS);
    assert_raw_row(&conn, OWNER_A, 1);
}

#[test]
fn core_autocommit_live_lease_returns_lease_busy_in_wal_journal() {
    let db = DbFile::fresh();
    let conn = open_core_conn(&db.path, JournalMode::Wal, Duration::from_millis(0));

    assert_core_claim(
        claim(&conn, NAME, OWNER_A, NOW_MS, TTL_MS),
        Expect::Acquired,
    );
    assert_core_claim(
        claim(&conn, NAME, OWNER_B, NOW_MS + 1, TTL_MS),
        Expect::LeaseBusy,
    );

    assert_live_lease(&conn, NOW_MS + 1, OWNER_A, 1, NOW_MS + TTL_MS);
    assert_raw_row(&conn, OWNER_A, 1);
}

#[test]
fn core_deferred_begin_lock_upgrade_returns_sqlite_lock_class_in_delete_journal() {
    let db = DbFile::fresh();
    let conn_a = open_core_conn(&db.path, JournalMode::Delete, Duration::from_millis(0));
    let conn_b = open_core_conn(&db.path, JournalMode::Delete, Duration::from_millis(0));

    conn_a.execute_batch("BEGIN").expect("begin conn_a");
    conn_b.execute_batch("BEGIN").expect("begin conn_b");

    assert_core_claim(
        claim_in_tx(&conn_a, NAME, OWNER_A, NOW_MS, TTL_MS),
        Expect::Acquired,
    );
    assert_core_claim(
        claim_in_tx(&conn_b, NAME, OWNER_B, NOW_MS, TTL_MS),
        Expect::SqliteBusyOrLocked,
    );

    // Roll back the blocked deferred reader first; reversing this can make
    // the writer rollback wait on the still-open reader in some journal modes.
    conn_b.execute_batch("ROLLBACK").expect("rollback conn_b");
    conn_a.execute_batch("ROLLBACK").expect("rollback conn_a");
    assert_no_row(&conn_a);
}

#[test]
fn core_deferred_begin_lock_upgrade_returns_sqlite_lock_class_in_wal_journal() {
    let db = DbFile::fresh();
    let conn_a = open_core_conn(&db.path, JournalMode::Wal, Duration::from_millis(0));
    let conn_b = open_core_conn(&db.path, JournalMode::Wal, Duration::from_millis(0));

    conn_a.execute_batch("BEGIN").expect("begin conn_a");
    conn_b.execute_batch("BEGIN").expect("begin conn_b");

    assert_core_claim(
        claim_in_tx(&conn_a, NAME, OWNER_A, NOW_MS, TTL_MS),
        Expect::Acquired,
    );
    assert_core_claim(
        claim_in_tx(&conn_b, NAME, OWNER_B, NOW_MS, TTL_MS),
        Expect::SqliteBusyOrLocked,
    );

    // Roll back the blocked deferred reader first; reversing this can make
    // the writer rollback wait on the still-open reader in some journal modes.
    conn_b.execute_batch("ROLLBACK").expect("rollback conn_b");
    conn_a.execute_batch("ROLLBACK").expect("rollback conn_a");
    assert_no_row(&conn_a);
}

#[test]
fn core_deferred_begin_renew_in_tx_lock_upgrade_returns_sqlite_lock_class() {
    let db = DbFile::fresh();
    let conn_a = open_core_conn(&db.path, JournalMode::Delete, Duration::from_millis(0));
    let conn_b = open_core_conn(&db.path, JournalMode::Delete, Duration::from_millis(0));

    assert_core_claim(
        claim(&conn_a, NAME, OWNER_A, NOW_MS, TTL_MS),
        Expect::Acquired,
    );

    conn_a.execute_batch("BEGIN").expect("begin conn_a");
    conn_b.execute_batch("BEGIN").expect("begin conn_b");

    assert_core_claim(
        claim_in_tx(&conn_a, "janitor", OWNER_A, NOW_MS + 1, TTL_MS),
        Expect::Acquired,
    );
    assert_core_lock_failure(renew_in_tx(&conn_b, NAME, OWNER_A, NOW_MS + 2, TTL_MS));

    // Roll back the blocked deferred reader first; reversing this can make
    // the writer rollback wait on the still-open reader in some journal modes.
    conn_b.execute_batch("ROLLBACK").expect("rollback conn_b");
    conn_a.execute_batch("ROLLBACK").expect("rollback conn_a");
    assert_live_lease(&conn_a, NOW_MS + 2, OWNER_A, 1, NOW_MS + TTL_MS);
}

#[test]
fn core_deferred_begin_release_in_tx_lock_upgrade_returns_sqlite_lock_class() {
    let db = DbFile::fresh();
    let conn_a = open_core_conn(&db.path, JournalMode::Delete, Duration::from_millis(0));
    let conn_b = open_core_conn(&db.path, JournalMode::Delete, Duration::from_millis(0));

    assert_core_claim(
        claim(&conn_a, NAME, OWNER_A, NOW_MS, TTL_MS),
        Expect::Acquired,
    );

    conn_a.execute_batch("BEGIN").expect("begin conn_a");
    conn_b.execute_batch("BEGIN").expect("begin conn_b");

    assert_core_claim(
        claim_in_tx(&conn_a, "janitor", OWNER_A, NOW_MS + 1, TTL_MS),
        Expect::Acquired,
    );
    assert_core_lock_failure(release_in_tx(&conn_b, NAME, OWNER_A, NOW_MS + 2));

    // Roll back the blocked deferred reader first; reversing this can make
    // the writer rollback wait on the still-open reader in some journal modes.
    conn_b.execute_batch("ROLLBACK").expect("rollback conn_b");
    conn_a.execute_batch("ROLLBACK").expect("rollback conn_a");
    assert_live_lease(&conn_a, NOW_MS + 2, OWNER_A, 1, NOW_MS + TTL_MS);
}

#[test]
fn core_begin_immediate_takes_writer_intent_before_bouncer_mutation() {
    let db = DbFile::fresh();
    let conn_a = open_core_conn(&db.path, JournalMode::Delete, Duration::from_millis(0));
    let conn_b = open_core_conn(&db.path, JournalMode::Delete, Duration::from_millis(0));

    conn_a
        .execute_batch("BEGIN IMMEDIATE")
        .expect("begin immediate conn_a");
    assert_core_claim(
        claim_in_tx(&conn_a, NAME, OWNER_A, NOW_MS, TTL_MS),
        Expect::Acquired,
    );

    let err = conn_b
        .execute_batch("BEGIN IMMEDIATE")
        .expect_err("conn_b should fail before bouncer mutation");
    assert!(sqlite_error_is_busy_or_locked(&err), "got {err:?}");

    conn_a.execute_batch("ROLLBACK").expect("rollback conn_a");
    assert_no_row(&conn_a);
}

#[test]
fn core_savepoint_inside_outer_transaction_participates_without_nested_begin() {
    let db = DbFile::fresh();
    let conn = open_core_conn(&db.path, JournalMode::Delete, Duration::from_millis(0));

    conn.execute_batch("BEGIN")
        .expect("begin outer transaction");
    conn.execute_batch("SAVEPOINT lease_ops")
        .expect("open savepoint");
    assert_core_claim(
        claim_in_tx(&conn, NAME, OWNER_A, NOW_MS, TTL_MS),
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
fn core_busy_timeout_zero_fails_immediately_under_writer_contention() {
    let db = DbFile::fresh();
    let conn_a = open_core_conn(&db.path, JournalMode::Delete, Duration::from_millis(0));
    let conn_b = open_core_conn(&db.path, JournalMode::Delete, Duration::from_millis(0));

    conn_a
        .execute_batch("BEGIN IMMEDIATE")
        .expect("hold writer lock");

    let start = Instant::now();
    assert_core_claim(
        claim(&conn_b, NAME, OWNER_B, NOW_MS, TTL_MS),
        Expect::SqliteBusyOrLocked,
    );
    assert!(
        start.elapsed() < Duration::from_millis(250),
        "busy_timeout=0 should fail quickly, elapsed {:?}",
        start.elapsed()
    );

    conn_a.execute_batch("ROLLBACK").expect("rollback conn_a");
    assert_no_row(&conn_a);
}

#[test]
fn core_busy_timeout_nonzero_fails_after_bounded_wait_under_writer_contention() {
    let db = DbFile::fresh();
    let conn_a = open_core_conn(&db.path, JournalMode::Delete, Duration::from_millis(0));
    let conn_b = open_core_conn(&db.path, JournalMode::Delete, Duration::from_millis(50));

    conn_a
        .execute_batch("BEGIN IMMEDIATE")
        .expect("hold writer lock");

    let start = Instant::now();
    assert_core_claim(
        claim(&conn_b, NAME, OWNER_B, NOW_MS, TTL_MS),
        Expect::SqliteBusyOrLocked,
    );
    let elapsed = start.elapsed();
    assert!(
        elapsed >= Duration::from_millis(20),
        "busy_timeout=50ms should wait before failing, elapsed {elapsed:?}"
    );
    assert!(
        elapsed < Duration::from_secs(2),
        "busy_timeout=50ms should remain bounded, elapsed {elapsed:?}"
    );

    conn_a.execute_batch("ROLLBACK").expect("rollback conn_a");
    assert_no_row(&conn_a);
}

#[test]
fn sql_autocommit_live_lease_returns_lease_busy_in_delete_journal() {
    let db = DbFile::fresh();
    let conn = open_sql_conn(&db.path, JournalMode::Delete, Duration::from_millis(0));

    assert_sql_claim(
        sql_claim(&conn, NAME, OWNER_A, NOW_MS, TTL_MS),
        Expect::Acquired,
    );
    assert_sql_claim(
        sql_claim(&conn, NAME, OWNER_B, NOW_MS + 1, TTL_MS),
        Expect::LeaseBusy,
    );

    assert_live_lease(&conn, NOW_MS + 1, OWNER_A, 1, NOW_MS + TTL_MS);
    assert_raw_row(&conn, OWNER_A, 1);
}

#[test]
fn sql_autocommit_live_lease_returns_lease_busy_in_wal_journal() {
    let db = DbFile::fresh();
    let conn = open_sql_conn(&db.path, JournalMode::Wal, Duration::from_millis(0));

    assert_sql_claim(
        sql_claim(&conn, NAME, OWNER_A, NOW_MS, TTL_MS),
        Expect::Acquired,
    );
    assert_sql_claim(
        sql_claim(&conn, NAME, OWNER_B, NOW_MS + 1, TTL_MS),
        Expect::LeaseBusy,
    );

    assert_live_lease(&conn, NOW_MS + 1, OWNER_A, 1, NOW_MS + TTL_MS);
    assert_raw_row(&conn, OWNER_A, 1);
}

#[test]
fn sql_deferred_begin_lock_upgrade_returns_sqlite_lock_class() {
    let db = DbFile::fresh();
    let conn_a = open_sql_conn(&db.path, JournalMode::Delete, Duration::from_millis(0));
    let conn_b = open_sql_conn(&db.path, JournalMode::Delete, Duration::from_millis(0));

    conn_a.execute_batch("BEGIN").expect("begin conn_a");
    conn_b.execute_batch("BEGIN").expect("begin conn_b");

    assert_sql_claim(
        sql_claim(&conn_a, NAME, OWNER_A, NOW_MS, TTL_MS),
        Expect::Acquired,
    );
    assert_sql_claim(
        sql_claim(&conn_b, NAME, OWNER_B, NOW_MS, TTL_MS),
        Expect::SqliteUdfUnknownWithBusyText,
    );

    // Roll back the blocked deferred reader first; reversing this can make
    // the writer rollback wait on the still-open reader in some journal modes.
    conn_b.execute_batch("ROLLBACK").expect("rollback conn_b");
    conn_a.execute_batch("ROLLBACK").expect("rollback conn_a");
    assert_no_row(&conn_a);
}

#[test]
fn sql_begin_immediate_takes_writer_intent_before_bouncer_mutation() {
    let db = DbFile::fresh();
    let conn_a = open_sql_conn(&db.path, JournalMode::Delete, Duration::from_millis(0));
    let conn_b = open_sql_conn(&db.path, JournalMode::Delete, Duration::from_millis(0));

    conn_a
        .execute_batch("BEGIN IMMEDIATE")
        .expect("begin immediate conn_a");
    assert_sql_claim(
        sql_claim(&conn_a, NAME, OWNER_A, NOW_MS, TTL_MS),
        Expect::Acquired,
    );

    let err = conn_b
        .execute_batch("BEGIN IMMEDIATE")
        .expect_err("conn_b should fail before bouncer mutation");
    assert!(sqlite_error_is_busy_or_locked(&err), "got {err:?}");

    conn_a.execute_batch("ROLLBACK").expect("rollback conn_a");
    assert_no_row(&conn_a);
}

#[test]
fn sql_savepoint_inside_outer_transaction_participates_without_nested_begin() {
    let db = DbFile::fresh();
    let conn = open_sql_conn(&db.path, JournalMode::Delete, Duration::from_millis(0));

    conn.execute_batch("BEGIN")
        .expect("begin outer transaction");
    conn.execute_batch("SAVEPOINT lease_ops")
        .expect("open savepoint");
    assert_sql_claim(
        sql_claim(&conn, NAME, OWNER_A, NOW_MS, TTL_MS),
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
