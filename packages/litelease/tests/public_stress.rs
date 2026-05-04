//! Phase 016 public-surface stress suite.
//!
//! This widens proof intensity beyond the narrative user journeys while
//! staying on shipped public surfaces only.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use litelease::{Litelease, LiteleaseRef, ClaimResult, ReleaseResult, RenewResult};
use litelease_core::attach_litelease_functions;
use rusqlite::{params, Connection};
use tempfile::TempDir;

const NAME: &str = "scheduler";
const TTL: Duration = Duration::from_secs(30);
const TTL_MS: i64 = 30_000;

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

fn sql_attach_and_bootstrap(conn: &Connection) {
    attach_litelease_functions(conn).expect("attach litelease sql functions");
    let bootstrapped: i64 = conn
        .query_row("SELECT litelease_bootstrap()", [], |row| row.get(0))
        .expect("sql bootstrap");
    assert_eq!(bootstrapped, 1);
}

fn sql_claim(conn: &Connection, name: &str, owner: &str, ttl_ms: i64, now_ms: i64) -> Option<i64> {
    conn.query_row(
        "SELECT litelease_claim(?1, ?2, ?3, ?4)",
        params![name, owner, ttl_ms, now_ms],
        |row| row.get(0),
    )
    .expect("sql claim")
}

fn sql_release(conn: &Connection, name: &str, owner: &str, now_ms: i64) -> i64 {
    conn.query_row(
        "SELECT litelease_release(?1, ?2, ?3)",
        params![name, owner, now_ms],
        |row| row.get(0),
    )
    .expect("sql release")
}

fn sql_owner(conn: &Connection, name: &str, now_ms: i64) -> Option<String> {
    conn.query_row(
        "SELECT litelease_owner(?1, ?2)",
        params![name, now_ms],
        |row| row.get(0),
    )
    .expect("sql owner")
}

fn system_now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_millis()
        .try_into()
        .expect("system time fits in i64")
}

#[test]
fn public_stress_repeated_wrapper_lifecycle_busy_renew_release_and_reclaim() {
    for iteration in 0..48 {
        let db = DbFile::fresh();
        let owner_a = format!("worker-a-{iteration}");
        let owner_b = format!("worker-b-{iteration}");

        let wrapper_a = Litelease::open(&db.path).expect("open wrapper_a");
        wrapper_a.bootstrap().expect("bootstrap");

        let first = wrapper_a
            .claim(NAME, &owner_a, TTL)
            .expect("first claim");
        let first = match first {
            ClaimResult::Acquired(lease) => lease,
            other => panic!("expected acquired first claim, got {other:?}"),
        };
        assert_eq!(first.owner, owner_a);
        assert_eq!(first.token, 1);

        let wrapper_b = Litelease::open(&db.path).expect("open wrapper_b");
        let second = wrapper_b
            .claim(NAME, &owner_b, TTL)
            .expect("second claim");
        match second {
            ClaimResult::Busy(current) => {
                assert_eq!(current.owner, owner_a);
                assert_eq!(current.token, 1);
            }
            other => panic!("expected Busy on second claim, got {other:?}"),
        }

        let renewed = wrapper_a
            .renew(NAME, &owner_a, TTL)
            .expect("renew current owner");
        match renewed {
            RenewResult::Renewed(lease) => {
                assert_eq!(lease.owner, owner_a);
                assert_eq!(lease.token, 1);
            }
            other => panic!("expected Renewed, got {other:?}"),
        }

        let released = wrapper_a
            .release(NAME, &owner_a)
            .expect("release");
        match released {
            ReleaseResult::Released { token, .. } => assert_eq!(token, 1),
            other => panic!("expected Released, got {other:?}"),
        }

        let after_release = wrapper_a.inspect(NAME).expect("inspect after release");
        assert!(
            after_release.is_none(),
            "released lease should no longer be live"
        );

        let reclaimed = wrapper_b
            .claim(NAME, &owner_b, TTL)
            .expect("reclaim after release");
        let reclaimed = match reclaimed {
            ClaimResult::Acquired(lease) => lease,
            other => panic!("expected acquired reclaim, got {other:?}"),
        };
        assert_eq!(reclaimed.owner, owner_b);
        assert_eq!(reclaimed.token, 2);
    }
}

#[test]
fn public_stress_repeated_sql_expiry_and_wrapper_sql_visibility() {
    for iteration in 0..40 {
        let db = DbFile::fresh();
        let owner_a = format!("sql-a-{iteration}");
        let owner_b = format!("sql-b-{iteration}");

        let sql_conn = fresh_conn(&db.path);
        sql_attach_and_bootstrap(&sql_conn);

        let now_ms = system_now_ms();
        let live_ttl_ms = 86_400_000;

        let first = sql_claim(&sql_conn, NAME, &owner_a, live_ttl_ms, now_ms)
            .expect("sql first claim");
        assert_eq!(first, 1);

        let wrapper = Litelease::open(&db.path).expect("open wrapper");
        let lease = wrapper.inspect(NAME).expect("wrapper inspect");
        let lease = lease.expect("wrapper should see sql-created live lease");
        assert_eq!(lease.owner, owner_a);
        assert_eq!(lease.token, 1);

        let released = wrapper
            .release(NAME, &owner_a)
            .expect("wrapper release sql-owned lease");
        assert!(matches!(released, ReleaseResult::Released { token: 1, .. }));

        let owner_after_release = sql_owner(&sql_conn, NAME, now_ms + 1);
        assert!(
            owner_after_release.is_none(),
            "sql should see no live owner after wrapper release"
        );

        let reclaimed = sql_claim(&sql_conn, NAME, &owner_b, live_ttl_ms, now_ms + 2)
            .expect("sql reclaim after release");
        assert_eq!(reclaimed, 2);

        let lease_after = wrapper.inspect(NAME).expect("wrapper inspect after reclaim");
        let lease_after = lease_after.expect("wrapper should see reclaimed lease");
        assert_eq!(lease_after.owner, owner_b);
        assert_eq!(lease_after.token, 2);

        let expiry_name = format!("expiry-{iteration}");
        let expiry_first = sql_claim(&sql_conn, &expiry_name, &owner_a, 100, 1_000)
            .expect("explicit-time first claim");
        assert_eq!(expiry_first, 1);

        let expiry_reclaim = sql_claim(&sql_conn, &expiry_name, &owner_b, 100, 1_101)
            .expect("explicit-time reclaim after expiry");
        assert_eq!(expiry_reclaim, 2);
    }
}

#[test]
fn public_stress_repeated_caller_owned_transaction_atomic_visibility() {
    for iteration in 0..32 {
        let db = DbFile::fresh();
        let owner = format!("tx-owner-{iteration}");

        let conn = fresh_conn(&db.path);
        let borrowed = LiteleaseRef::new(&conn);
        borrowed.bootstrap().expect("bootstrap");

        conn.execute_batch("CREATE TABLE jobs (payload TEXT NOT NULL)")
            .expect("create jobs table");
        conn.execute_batch("BEGIN IMMEDIATE")
            .expect("begin immediate transaction");
        conn.execute(
            "INSERT INTO jobs(payload) VALUES (?1)",
            params![format!("work-{iteration}")],
        )
        .expect("insert business row");

        let claim = borrowed
            .claim(NAME, &owner, TTL)
            .expect("claim inside caller-owned tx");
        assert!(
            matches!(claim, ClaimResult::Acquired(_)),
            "claim should succeed inside caller-owned transaction"
        );

        let observer = fresh_conn(&db.path);
        let pre_commit_jobs: i64 = observer
            .query_row("SELECT COUNT(*) FROM jobs", [], |row| row.get(0))
            .expect("count jobs before commit");
        assert_eq!(pre_commit_jobs, 0);

        let observer_borrowed = LiteleaseRef::new(&observer);
        let pre_commit_lease = observer_borrowed
            .inspect(NAME)
            .expect("inspect before commit");
        assert!(pre_commit_lease.is_none());

        conn.execute_batch("COMMIT").expect("commit transaction");

        let observer_after = fresh_conn(&db.path);
        let post_commit_jobs: i64 = observer_after
            .query_row("SELECT COUNT(*) FROM jobs", [], |row| row.get(0))
            .expect("count jobs after commit");
        assert_eq!(post_commit_jobs, 1);

        let observer_borrowed = LiteleaseRef::new(&observer_after);
        let post_commit_lease = observer_borrowed
            .inspect(NAME)
            .expect("inspect after commit");
        let post_commit_lease = post_commit_lease.expect("lease should be visible after commit");
        assert_eq!(post_commit_lease.owner, owner);
        assert_eq!(post_commit_lease.token, 1);
    }
}

#[test]
fn public_stress_release_shaped_sql_smoke_repeats_cleanly() {
    for iteration in 0..24 {
        let db = DbFile::fresh();
        let conn = fresh_conn(&db.path);
        sql_attach_and_bootstrap(&conn);

        let owner = format!("smoke-owner-{iteration}");
        let token = sql_claim(&conn, NAME, &owner, TTL_MS, 10_000)
            .expect("claim via sql smoke path");
        assert_eq!(token, 1);

        let owner_visible = sql_owner(&conn, NAME, 10_001).expect("owner visible");
        assert_eq!(owner_visible, owner);

        let released = sql_release(&conn, NAME, &owner, 10_002);
        assert_eq!(released, 1);

        let owner_after = sql_owner(&conn, NAME, 10_003);
        assert!(owner_after.is_none());
    }
}
