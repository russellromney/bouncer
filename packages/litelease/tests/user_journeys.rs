//! Phase 015 user-journey acceptance suite.
//!
//! Proves Litelease behaves the way a normal user would expect
//! when using shipped public surfaces on one real SQLite file.
//!
//! Journeys (matching spec-diff.md):
//! 1. Fresh bootstrap + first claim succeeds
//! 2. Second independent caller sees lease busy
//! 3. Release then reclaim increments token
//! 4. Expiry then reclaim increments token (SQL surface, explicit now_ms)
//! 5. Cross-surface interoperability on one file (Rust wrapper ↔ SQL extension)
//! 6. Caller-owned transaction: business write + lease mutation visible together after commit
//! 7. Drifted schema fails loudly through public bootstrap surfaces

use std::path::{Path, PathBuf};
use std::time::Duration;

use litelease::{Litelease, LiteleaseRef, ClaimResult, ReleaseResult};
use litelease_core::{attach_litelease_functions, bootstrap_litelease_schema};
use rusqlite::{params, Connection, OptionalExtension};
use tempfile::TempDir;

const NAME: &str = "scheduler";
const OWNER_A: &str = "worker-a";
const OWNER_B: &str = "worker-b";

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

fn fresh_conn(path: &Path) -> Connection {
    Connection::open(path).expect("open fresh connection")
}

fn sql_claim(conn: &Connection, name: &str, owner: &str, ttl_ms: i64, now_ms: i64) -> Option<i64> {
    conn.query_row(
        "SELECT litelease_claim(?1, ?2, ?3, ?4)",
        params![name, owner, ttl_ms, now_ms],
        |row| row.get(0),
    )
    .expect("sql claim")
}

fn sql_owner(conn: &Connection, name: &str, now_ms: i64) -> Option<String> {
    conn.query_row(
        "SELECT litelease_owner(?1, ?2)",
        params![name, now_ms],
        |row| row.get(0),
    )
    .expect("sql owner")
}

// -----------------------------------------------------------------------
// Journey 1
// -----------------------------------------------------------------------

#[test]
fn user_journey_001_bootstrap_and_first_claim() {
    let db = DbFile::fresh();
    let wrapper = Litelease::open(&db.path).expect("open wrapper");
    wrapper.bootstrap().expect("bootstrap fresh file");

    let claim = wrapper
        .claim(NAME, OWNER_A, Duration::from_secs(30))
        .expect("first claim");
    assert!(
        matches!(claim, ClaimResult::Acquired(_)),
        "first claim should succeed on a fresh bootstrapped file"
    );
}

// -----------------------------------------------------------------------
// Journey 2
// -----------------------------------------------------------------------

#[test]
fn user_journey_002_second_caller_sees_busy() {
    let db = DbFile::fresh();
    let wrapper_a = Litelease::open(&db.path).expect("open wrapper_a");
    wrapper_a.bootstrap().expect("bootstrap");

    let claim_a = wrapper_a
        .claim(NAME, OWNER_A, Duration::from_secs(30))
        .expect("first claim");
    assert!(matches!(claim_a, ClaimResult::Acquired(_)));

    // Independent second caller on a separate connection.
    let wrapper_b = Litelease::open(&db.path).expect("open wrapper_b");
    let claim_b = wrapper_b
        .claim(NAME, OWNER_B, Duration::from_secs(30))
        .expect("second claim");
    assert!(
        matches!(claim_b, ClaimResult::Busy(_)),
        "second caller should see lease busy, not false success"
    );

    match claim_b {
        ClaimResult::Busy(current) => assert_eq!(current.owner, OWNER_A),
        _ => panic!("expected Busy result carrying current owner"),
    }
}

// -----------------------------------------------------------------------
// Journey 3
// -----------------------------------------------------------------------

#[test]
fn user_journey_003_release_then_reclaim_increments_token() {
    let db = DbFile::fresh();
    let wrapper_a = Litelease::open(&db.path).expect("open wrapper_a");
    wrapper_a.bootstrap().expect("bootstrap");

    let token_a = match wrapper_a
        .claim(NAME, OWNER_A, Duration::from_secs(30))
        .expect("first claim")
    {
        ClaimResult::Acquired(lease) => lease.token,
        _ => panic!("expected acquired"),
    };

    let released = wrapper_a
        .release(NAME, OWNER_A)
        .expect("release");
    assert!(matches!(released, ReleaseResult::Released { .. }));

    let wrapper_b = Litelease::open(&db.path).expect("open wrapper_b");
    let token_b = match wrapper_b
        .claim(NAME, OWNER_B, Duration::from_secs(30))
        .expect("reclaim")
    {
        ClaimResult::Acquired(lease) => lease.token,
        _ => panic!("expected acquired after reclaim"),
    };

    assert!(
        token_b > token_a,
        "token should increase after release and reclaim: {} -> {}",
        token_a,
        token_b
    );
}

// -----------------------------------------------------------------------
// Journey 4: explicit now_ms via SQL surface so expiry is deterministic
// -----------------------------------------------------------------------

#[test]
fn user_journey_004_expiry_then_reclaim_increments_token() {
    let db = DbFile::fresh();
    let conn = fresh_conn(&db.path);
    attach_litelease_functions(&conn).expect("attach litelease sql functions");
    bootstrap_litelease_schema(&conn).expect("bootstrap");

    let now_ms: i64 = 1_000;
    let ttl_ms: i64 = 500;

    let token_a = sql_claim(&conn, NAME, OWNER_A, ttl_ms, now_ms)
        .expect("first claim should succeed");
    assert_eq!(token_a, 1);

    let token_b = sql_claim(&conn, NAME, OWNER_B, ttl_ms, now_ms + ttl_ms + 1)
        .expect("reclaim after expiry should succeed");
    assert_eq!(
        token_b, 2,
        "token should increment after expiry and reclaim"
    );

    let owner = sql_owner(&conn, NAME, now_ms + ttl_ms + 2)
        .expect("owner should exist after reclaim");
    assert_eq!(owner, OWNER_B);
}

// -----------------------------------------------------------------------
// Journey 5: Rust wrapper ↔ SQL extension on one file
// -----------------------------------------------------------------------

#[test]
fn user_journey_005_cross_surface_interop() {
    let db = DbFile::fresh();

    let sql_conn = fresh_conn(&db.path);
    attach_litelease_functions(&sql_conn).expect("attach sql functions");
    bootstrap_litelease_schema(&sql_conn).expect("bootstrap via sql");

    let now_ms: i64 = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;
    let ttl_ms: i64 = 86_400_000; // 24 hours so the lease stays live
    let token = sql_claim(&sql_conn, NAME, OWNER_A, ttl_ms, now_ms)
        .expect("sql claim should succeed");
    assert_eq!(token, 1);

    let wrapper = Litelease::open(&db.path).expect("open wrapper");
    let lease = wrapper.inspect(NAME).expect("wrapper inspect");
    assert!(lease.is_some(), "wrapper should see sql-created lease");
    let lease = lease.unwrap();
    assert_eq!(lease.owner, OWNER_A);
    assert_eq!(lease.token, token);

    let released = wrapper
        .release(NAME, OWNER_A)
        .expect("wrapper release");
    assert!(matches!(released, ReleaseResult::Released { .. }));

    let owner_after = sql_owner(&sql_conn, NAME, now_ms + 1);
    assert!(
        owner_after.is_none(),
        "sql should see no owner after wrapper release"
    );

    let new_token = sql_claim(&sql_conn, NAME, OWNER_B, ttl_ms, now_ms + 2)
        .expect("sql reclaim should succeed");
    assert_eq!(new_token, 2, "token should increment after reclaim");

    let new_lease = wrapper.inspect(NAME).expect("wrapper inspect after reclaim");
    assert!(new_lease.is_some());
    assert_eq!(new_lease.unwrap().owner, OWNER_B);
}

// -----------------------------------------------------------------------
// Journey 6: caller-owned transaction atomic visibility
// -----------------------------------------------------------------------

#[test]
fn user_journey_006_caller_owned_transaction_atomic_visibility() {
    let db = DbFile::fresh();
    let conn = fresh_conn(&db.path);
    bootstrap_litelease_schema(&conn).expect("bootstrap");

    conn.execute_batch("CREATE TABLE jobs (payload TEXT NOT NULL)")
        .expect("create business table");

    conn.execute_batch("BEGIN").expect("begin transaction");

    conn.execute(
        "INSERT INTO jobs(payload) VALUES (?1)",
        params!["work unit"],
    )
    .expect("insert business row");

    let borrowed = LiteleaseRef::new(&conn);
    let claim = borrowed
        .claim(NAME, OWNER_A, Duration::from_secs(30))
        .expect("claim inside transaction");
    assert!(
        matches!(claim, ClaimResult::Acquired(_)),
        "claim should succeed inside transaction"
    );

    let observer = fresh_conn(&db.path);
    let job_count: i64 = observer
        .query_row("SELECT COUNT(*) FROM jobs", [], |row| row.get(0))
        .expect("count jobs");
    assert_eq!(job_count, 0, "observer should not see uncommitted business write");

    let lease: Option<String> = observer
        .query_row(
            "SELECT owner FROM litelease_resources WHERE name = ?1",
            params![NAME],
            |row| row.get::<_, Option<String>>(0),
        )
        .optional()
        .expect("read litelease_resources")
        .flatten();
    assert!(
        lease.is_none(),
        "observer should not see uncommitted lease mutation"
    );

    conn.execute_batch("COMMIT").expect("commit transaction");

    let observer2 = fresh_conn(&db.path);
    let job_count_after: i64 = observer2
        .query_row("SELECT COUNT(*) FROM jobs", [], |row| row.get(0))
        .expect("count jobs after commit");
    assert_eq!(
        job_count_after, 1,
        "fresh observer should see committed business write"
    );

    let observer_borrowed = LiteleaseRef::new(&observer2);
    let lease_after = observer_borrowed
        .inspect(NAME)
        .expect("fresh observer inspect after commit");
    assert!(
        lease_after.is_some(),
        "fresh observer should see committed lease"
    );
    assert_eq!(lease_after.unwrap().owner, OWNER_A);
}

// -----------------------------------------------------------------------
// Journey 7: drifted schema fails loudly on public bootstrap surfaces
// -----------------------------------------------------------------------

#[test]
fn user_journey_007_drifted_schema_fails_loudly() {
    let db = DbFile::fresh();
    let conn = fresh_conn(&db.path);

    conn.execute_batch(
        "CREATE TABLE litelease_resources (
           name TEXT PRIMARY KEY,
           owner TEXT,
           token INTEGER NOT NULL,
           lease_expires_at_ms INTEGER,
           created_at_ms INTEGER NOT NULL
         );",
    )
    .expect("create drifted schema");

    let wrapper = Litelease::open(&db.path).expect("open wrapper");
    let err = wrapper
        .bootstrap()
        .expect_err("wrapper bootstrap should fail on drifted schema");
    assert!(
        err.to_string().contains("schema mismatch")
            || err.to_string().contains("SchemaMismatch"),
        "wrapper should report SchemaMismatch, got: {err}"
    );

    let sql_conn = fresh_conn(&db.path);
    attach_litelease_functions(&sql_conn).expect("attach functions");
    let result: rusqlite::Result<i64> = sql_conn
        .query_row("SELECT litelease_bootstrap()", [], |row| row.get(0));
    assert!(
        result.is_err(),
        "sql bootstrap should fail on drifted schema"
    );
}
