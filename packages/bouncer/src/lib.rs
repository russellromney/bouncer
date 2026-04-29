use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use bouncer_honker as core;
use rusqlite::{Connection, Transaction as SqlTransaction, TransactionBehavior};

pub use core::{ClaimResult, LeaseInfo, ReleaseResult, RenewResult};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("database error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("bouncer core error: {0}")]
    Core(#[from] core::Error),
    #[error("system clock error: {0}")]
    SystemTime(#[from] std::time::SystemTimeError),
    #[error("system time {0}ms since unix epoch does not fit in i64")]
    SystemTimeTooLarge(u128),
    #[error("duration {0:?} is too large to fit in i64 milliseconds")]
    DurationTooLarge(Duration),
}

#[derive(Debug)]
pub struct Bouncer {
    conn: Connection,
}

#[derive(Debug, Clone, Copy)]
pub struct BouncerRef<'a> {
    conn: &'a Connection,
}

#[derive(Debug)]
pub struct Transaction<'db> {
    tx: SqlTransaction<'db>,
}

impl Bouncer {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let conn = Connection::open(path)?;
        Ok(Self { conn })
    }

    pub fn bootstrap(&self) -> Result<()> {
        core::bootstrap_bouncer_schema(&self.conn)?;
        Ok(())
    }

    pub fn borrowed(&self) -> BouncerRef<'_> {
        BouncerRef::new(&self.conn)
    }

    /// Begin a `BEGIN IMMEDIATE` transaction for atomic business writes
    /// plus Bouncer lease mutations on this connection.
    pub fn transaction(&mut self) -> Result<Transaction<'_>> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        Ok(Transaction { tx })
    }

    pub fn inspect(&self, name: &str) -> Result<Option<LeaseInfo>> {
        self.borrowed().inspect(name)
    }

    pub fn claim(&self, name: &str, owner: &str, ttl: Duration) -> Result<ClaimResult> {
        self.borrowed().claim(name, owner, ttl)
    }

    pub fn renew(&self, name: &str, owner: &str, ttl: Duration) -> Result<RenewResult> {
        self.borrowed().renew(name, owner, ttl)
    }

    pub fn release(&self, name: &str, owner: &str) -> Result<ReleaseResult> {
        self.borrowed().release(name, owner)
    }
}

impl<'a> BouncerRef<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    pub fn bootstrap(&self) -> Result<()> {
        core::bootstrap_bouncer_schema(self.conn)?;
        Ok(())
    }

    pub fn inspect(&self, name: &str) -> Result<Option<LeaseInfo>> {
        core::inspect(self.conn, name, system_now_ms()?).map_err(Error::from)
    }

    pub fn claim(&self, name: &str, owner: &str, ttl: Duration) -> Result<ClaimResult> {
        let now_ms = system_now_ms()?;
        let ttl_ms = duration_to_ttl_ms(ttl)?;

        (if self.conn.is_autocommit() {
            core::claim(self.conn, name, owner, now_ms, ttl_ms)
        } else {
            core::claim_in_tx(self.conn, name, owner, now_ms, ttl_ms)
        })
        .map_err(Error::from)
    }

    pub fn renew(&self, name: &str, owner: &str, ttl: Duration) -> Result<RenewResult> {
        let now_ms = system_now_ms()?;
        let ttl_ms = duration_to_ttl_ms(ttl)?;

        (if self.conn.is_autocommit() {
            core::renew(self.conn, name, owner, now_ms, ttl_ms)
        } else {
            core::renew_in_tx(self.conn, name, owner, now_ms, ttl_ms)
        })
        .map_err(Error::from)
    }

    pub fn release(&self, name: &str, owner: &str) -> Result<ReleaseResult> {
        let now_ms = system_now_ms()?;

        (if self.conn.is_autocommit() {
            core::release(self.conn, name, owner, now_ms)
        } else {
            core::release_in_tx(self.conn, name, owner, now_ms)
        })
        .map_err(Error::from)
    }
}

impl<'db> Transaction<'db> {
    pub fn conn(&self) -> &Connection {
        &self.tx
    }

    pub fn inspect(&self, name: &str) -> Result<Option<LeaseInfo>> {
        core::inspect(self.conn(), name, system_now_ms()?).map_err(Error::from)
    }

    pub fn claim(&self, name: &str, owner: &str, ttl: Duration) -> Result<ClaimResult> {
        core::claim_in_tx(
            self.conn(),
            name,
            owner,
            system_now_ms()?,
            duration_to_ttl_ms(ttl)?,
        )
        .map_err(Error::from)
    }

    pub fn renew(&self, name: &str, owner: &str, ttl: Duration) -> Result<RenewResult> {
        core::renew_in_tx(
            self.conn(),
            name,
            owner,
            system_now_ms()?,
            duration_to_ttl_ms(ttl)?,
        )
        .map_err(Error::from)
    }

    pub fn release(&self, name: &str, owner: &str) -> Result<ReleaseResult> {
        core::release_in_tx(self.conn(), name, owner, system_now_ms()?).map_err(Error::from)
    }

    pub fn commit(self) -> Result<()> {
        self.tx.commit()?;
        Ok(())
    }

    pub fn rollback(self) -> Result<()> {
        self.tx.rollback()?;
        Ok(())
    }
}

fn system_now_ms() -> Result<i64> {
    let millis = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();
    i64::try_from(millis).map_err(|_| Error::SystemTimeTooLarge(millis))
}

fn duration_to_ttl_ms(ttl: Duration) -> Result<i64> {
    let millis = ttl.as_millis();
    i64::try_from(millis).map_err(|_| Error::DurationTooLarge(ttl))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::params;
    use tempfile::TempDir;

    fn open_wrapper_db() -> (TempDir, Bouncer) {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let db_path = tempdir.path().join("bouncer.sqlite3");
        let wrapper = Bouncer::open(&db_path).expect("open wrapper db");
        (tempdir, wrapper)
    }

    fn configure_test_connection(conn: &Connection) {
        // Test-only harness config for multi-connection reliability.
        conn.pragma_update(None, "journal_mode", "WAL")
            .expect("set WAL mode");
        conn.busy_timeout(Duration::from_secs(1))
            .expect("set busy timeout");
    }

    fn core_missing_schema_error(err: &Error) -> bool {
        match err {
            Error::Core(core::Error::Sqlite(rusqlite::Error::SqliteFailure(_, Some(message)))) => {
                message.contains("no such table") && message.contains("bouncer_resources")
            }
            _ => false,
        }
    }

    fn system_now_for_core() -> i64 {
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time after unix epoch")
            .as_millis();
        i64::try_from(millis).expect("time fits in i64 milliseconds")
    }

    fn attach_sql_functions(conn: &Connection) {
        core::attach_bouncer_functions(conn).expect("attach bouncer sql functions");
    }

    fn open_sql_conn(tempdir: &TempDir) -> Connection {
        let conn = Connection::open(tempdir.path().join("bouncer.sqlite3")).expect("open sql conn");
        configure_test_connection(&conn);
        attach_sql_functions(&conn);
        conn
    }

    fn create_business_table(conn: &Connection) {
        conn.execute_batch(
            "CREATE TABLE business_events (
               id INTEGER PRIMARY KEY,
               note TEXT NOT NULL
             );",
        )
        .expect("create business_events");
    }

    fn business_event_count(conn: &Connection) -> i64 {
        conn.query_row("SELECT COUNT(*) FROM business_events", [], |row| row.get(0))
            .expect("count business events")
    }

    fn sql_missing_schema_error(err: &rusqlite::Error) -> bool {
        match err {
            rusqlite::Error::SqliteFailure(_, Some(message)) => {
                message.contains("no such table") && message.contains("bouncer_resources")
            }
            _ => false,
        }
    }

    fn sql_bootstrap(conn: &Connection) -> rusqlite::Result<i64> {
        conn.query_row("SELECT bouncer_bootstrap()", [], |row| row.get(0))
    }

    fn sql_claim(
        conn: &Connection,
        name: &str,
        owner: &str,
        ttl_ms: i64,
        now_ms: i64,
    ) -> rusqlite::Result<Option<i64>> {
        conn.query_row(
            "SELECT bouncer_claim(?1, ?2, ?3, ?4)",
            params![name, owner, ttl_ms, now_ms],
            |row| row.get(0),
        )
    }

    fn sql_renew(
        conn: &Connection,
        name: &str,
        owner: &str,
        ttl_ms: i64,
        now_ms: i64,
    ) -> rusqlite::Result<Option<i64>> {
        conn.query_row(
            "SELECT bouncer_renew(?1, ?2, ?3, ?4)",
            params![name, owner, ttl_ms, now_ms],
            |row| row.get(0),
        )
    }

    fn sql_release(
        conn: &Connection,
        name: &str,
        owner: &str,
        now_ms: i64,
    ) -> rusqlite::Result<i64> {
        conn.query_row(
            "SELECT bouncer_release(?1, ?2, ?3)",
            params![name, owner, now_ms],
            |row| row.get(0),
        )
    }

    fn sql_owner(conn: &Connection, name: &str, now_ms: i64) -> rusqlite::Result<Option<String>> {
        conn.query_row(
            "SELECT bouncer_owner(?1, ?2)",
            params![name, now_ms],
            |row| row.get(0),
        )
    }

    fn sql_token(conn: &Connection, name: &str) -> rusqlite::Result<Option<i64>> {
        conn.query_row("SELECT bouncer_token(?1)", params![name], |row| row.get(0))
    }

    #[test]
    fn open_does_not_bootstrap_implicitly() {
        let (_tempdir, wrapper) = open_wrapper_db();

        let err = wrapper
            .claim("scheduler", "worker-a", Duration::from_secs(1))
            .expect_err("claim before bootstrap should fail");

        assert!(core_missing_schema_error(&err));
    }

    #[test]
    fn wrapper_methods_fail_cleanly_before_bootstrap() {
        let (_tempdir, wrapper) = open_wrapper_db();

        let err = wrapper
            .inspect("scheduler")
            .expect_err("inspect before bootstrap should fail");

        assert!(core_missing_schema_error(&err));
    }

    #[test]
    fn bootstrap_is_explicit_and_idempotent() {
        let (_tempdir, wrapper) = open_wrapper_db();

        wrapper.bootstrap().expect("first bootstrap");
        wrapper.bootstrap().expect("second bootstrap");

        let acquired = wrapper
            .claim("scheduler", "worker-a", Duration::from_secs(5))
            .expect("claim after bootstrap");

        assert!(matches!(acquired, ClaimResult::Acquired(_)));
    }

    #[test]
    fn sql_functions_require_explicit_bootstrap() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let sql_conn = open_sql_conn(&tempdir);

        let err = sql_claim(&sql_conn, "scheduler", "worker-a", 1_000, 100)
            .expect_err("claim before bootstrap should fail");

        assert!(sql_missing_schema_error(&err));
    }

    #[test]
    fn sql_bootstrap_is_explicit_and_idempotent() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let sql_conn = open_sql_conn(&tempdir);

        assert_eq!(sql_bootstrap(&sql_conn).expect("first bootstrap"), 1);
        assert_eq!(sql_bootstrap(&sql_conn).expect("second bootstrap"), 1);

        assert_eq!(
            sql_claim(&sql_conn, "scheduler", "worker-a", 5_000, 100)
                .expect("claim after bootstrap"),
            Some(1)
        );
    }

    #[test]
    fn wrapper_performs_full_lease_cycle() {
        let (_tempdir, wrapper) = open_wrapper_db();
        wrapper.bootstrap().expect("bootstrap");

        let acquired = wrapper
            .claim("scheduler", "worker-a", Duration::from_secs(5))
            .expect("claim");
        let lease = match acquired {
            ClaimResult::Acquired(lease) => lease,
            ClaimResult::Busy(current) => panic!("unexpected busy lease: {current:?}"),
        };

        let inspected = wrapper.inspect("scheduler").expect("inspect");
        assert_eq!(inspected, Some(lease.clone()));

        let renewed = wrapper
            .renew("scheduler", "worker-a", Duration::from_secs(10))
            .expect("renew");
        let renewed = match renewed {
            RenewResult::Renewed(lease) => lease,
            RenewResult::Rejected { current } => {
                panic!("unexpected renew rejection: {current:?}")
            }
        };
        assert!(renewed.lease_expires_at_ms >= lease.lease_expires_at_ms);

        let released = wrapper.release("scheduler", "worker-a").expect("release");
        assert!(matches!(released, ReleaseResult::Released { .. }));
        assert_eq!(
            wrapper.inspect("scheduler").expect("inspect after release"),
            None
        );
    }

    #[test]
    fn wrapper_ttl_rejection_matches_core() {
        let (_tempdir, wrapper) = open_wrapper_db();
        wrapper.bootstrap().expect("bootstrap");

        let err = wrapper
            .claim("scheduler", "worker-a", Duration::ZERO)
            .expect_err("zero ttl should fail");

        match err {
            Error::Core(core::Error::InvalidTtlMs(0)) => {}
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn wrapper_claim_is_visible_to_core_on_separate_connection() {
        let (tempdir, wrapper) = open_wrapper_db();
        wrapper.bootstrap().expect("bootstrap");
        configure_test_connection(&wrapper.conn);

        let core_conn =
            Connection::open(tempdir.path().join("bouncer.sqlite3")).expect("open core conn");
        configure_test_connection(&core_conn);

        let lease = match wrapper
            .claim("scheduler", "worker-a", Duration::from_secs(30))
            .expect("wrapper claim")
        {
            ClaimResult::Acquired(lease) => lease,
            ClaimResult::Busy(current) => panic!("unexpected busy lease: {current:?}"),
        };

        let inspected = core::inspect(&core_conn, "scheduler", lease.lease_expires_at_ms - 1)
            .expect("core inspect");
        assert_eq!(inspected, Some(lease));
    }

    #[test]
    fn sql_claim_is_visible_to_wrapper_on_separate_connection() {
        let (tempdir, wrapper) = open_wrapper_db();
        configure_test_connection(&wrapper.conn);
        wrapper.bootstrap().expect("wrapper bootstrap");

        let sql_conn = open_sql_conn(&tempdir);
        assert_eq!(sql_bootstrap(&sql_conn).expect("sql bootstrap"), 1);
        let now_ms = system_now_for_core();

        assert_eq!(
            sql_claim(&sql_conn, "scheduler", "worker-a", 5_000, now_ms).expect("sql claim"),
            Some(1)
        );

        let inspected = wrapper.inspect("scheduler").expect("wrapper inspect");
        let inspected = inspected.expect("live lease");
        assert_eq!(inspected.owner, "worker-a");
        assert_eq!(inspected.token, 1);
    }

    #[test]
    fn wrapper_claim_is_visible_to_sql_on_separate_connection() {
        let (tempdir, wrapper) = open_wrapper_db();
        configure_test_connection(&wrapper.conn);
        wrapper.bootstrap().expect("wrapper bootstrap");

        let sql_conn = open_sql_conn(&tempdir);
        assert_eq!(sql_bootstrap(&sql_conn).expect("sql bootstrap"), 1);

        let lease = match wrapper
            .claim("scheduler", "worker-a", Duration::from_secs(30))
            .expect("wrapper claim")
        {
            ClaimResult::Acquired(lease) => lease,
            ClaimResult::Busy(current) => panic!("unexpected busy lease: {current:?}"),
        };

        assert_eq!(
            sql_owner(&sql_conn, "scheduler", lease.lease_expires_at_ms - 1).expect("sql owner"),
            Some("worker-a".to_owned())
        );
        assert_eq!(
            sql_token(&sql_conn, "scheduler").expect("sql token"),
            Some(1)
        );
    }

    #[test]
    fn sql_and_rust_preserve_monotonic_fencing_tokens() {
        let (tempdir, wrapper) = open_wrapper_db();
        configure_test_connection(&wrapper.conn);
        wrapper.bootstrap().expect("wrapper bootstrap");

        let sql_conn = open_sql_conn(&tempdir);
        assert_eq!(sql_bootstrap(&sql_conn).expect("sql bootstrap"), 1);
        let now_ms = system_now_for_core();

        assert_eq!(
            sql_claim(&sql_conn, "scheduler", "worker-a", 5_000, now_ms).expect("sql claim"),
            Some(1)
        );
        assert_eq!(
            wrapper
                .release("scheduler", "worker-a")
                .expect("wrapper release"),
            ReleaseResult::Released {
                name: "scheduler".to_owned(),
                token: 1,
            }
        );
        assert_eq!(
            sql_claim(&sql_conn, "scheduler", "worker-b", 5_000, now_ms + 200)
                .expect("sql reclaim"),
            Some(2)
        );
        assert_eq!(
            sql_token(&sql_conn, "scheduler").expect("sql token"),
            Some(2)
        );
    }

    #[test]
    fn sql_full_lease_cycle_matches_expected_return_shapes() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let sql_conn = open_sql_conn(&tempdir);
        assert_eq!(sql_bootstrap(&sql_conn).expect("sql bootstrap"), 1);

        assert_eq!(
            sql_claim(&sql_conn, "scheduler", "worker-a", 5_000, 100).expect("sql claim"),
            Some(1)
        );
        assert_eq!(
            sql_claim(&sql_conn, "scheduler", "worker-b", 5_000, 200).expect("busy sql claim"),
            None
        );
        assert_eq!(
            sql_renew(&sql_conn, "scheduler", "worker-a", 7_000, 300).expect("sql renew"),
            Some(1)
        );
        assert_eq!(
            sql_owner(&sql_conn, "scheduler", 301).expect("sql owner"),
            Some("worker-a".to_owned())
        );
        assert_eq!(
            sql_token(&sql_conn, "scheduler").expect("sql token"),
            Some(1)
        );
        assert_eq!(
            sql_release(&sql_conn, "scheduler", "worker-a", 400).expect("sql release"),
            1
        );
        assert_eq!(
            sql_owner(&sql_conn, "scheduler", 401).expect("sql owner after release"),
            None
        );
        assert_eq!(
            sql_token(&sql_conn, "scheduler").expect("sql token after release"),
            Some(1)
        );
    }

    #[test]
    fn core_claim_is_visible_to_wrapper_on_separate_connection() {
        let (tempdir, wrapper) = open_wrapper_db();
        wrapper.bootstrap().expect("bootstrap");
        configure_test_connection(&wrapper.conn);

        let core_conn =
            Connection::open(tempdir.path().join("bouncer.sqlite3")).expect("open core conn");
        configure_test_connection(&core_conn);

        let now_ms = system_now_for_core();
        let lease = match core::claim(&core_conn, "scheduler", "worker-a", now_ms, 60_000)
            .expect("core claim")
        {
            core::ClaimResult::Acquired(lease) => lease,
            core::ClaimResult::Busy(current) => panic!("unexpected busy lease: {current:?}"),
        };

        let inspected = wrapper.inspect("scheduler").expect("wrapper inspect");
        assert_eq!(inspected, Some(lease));
    }

    #[test]
    fn fencing_token_stays_monotonic_across_wrapper_and_core() {
        let (tempdir, wrapper) = open_wrapper_db();
        wrapper.bootstrap().expect("bootstrap");
        configure_test_connection(&wrapper.conn);

        let core_conn =
            Connection::open(tempdir.path().join("bouncer.sqlite3")).expect("open core conn");
        configure_test_connection(&core_conn);

        let first = match wrapper
            .claim("scheduler", "worker-a", Duration::from_millis(25))
            .expect("wrapper claim")
        {
            ClaimResult::Acquired(lease) => lease,
            ClaimResult::Busy(current) => panic!("unexpected busy lease: {current:?}"),
        };

        let second = match core::claim(
            &core_conn,
            "scheduler",
            "worker-b",
            first.lease_expires_at_ms + 1,
            25,
        )
        .expect("core takeover claim")
        {
            core::ClaimResult::Acquired(lease) => lease,
            core::ClaimResult::Busy(current) => panic!("unexpected busy lease: {current:?}"),
        };

        assert_eq!(second.token, first.token + 1);
    }

    #[test]
    fn borrowed_claim_and_renew_commit_with_explicit_transaction() {
        let (_tempdir, wrapper) = open_wrapper_db();
        wrapper.bootstrap().expect("bootstrap");
        create_business_table(&wrapper.conn);
        let borrowed = wrapper.borrowed();

        wrapper.conn.execute_batch("BEGIN").expect("begin tx");
        wrapper
            .conn
            .execute(
                "INSERT INTO business_events(note) VALUES (?1)",
                params!["borrowed commit"],
            )
            .expect("insert business event");

        let acquired = borrowed
            .claim("scheduler", "worker-a", Duration::from_secs(5))
            .expect("borrowed claim");
        let acquired = match acquired {
            ClaimResult::Acquired(lease) => lease,
            ClaimResult::Busy(current) => panic!("unexpected busy lease: {current:?}"),
        };

        let renewed = borrowed
            .renew("scheduler", "worker-a", Duration::from_secs(10))
            .expect("borrowed renew");
        let renewed = match renewed {
            RenewResult::Renewed(lease) => lease,
            RenewResult::Rejected { current } => {
                panic!("unexpected renew rejection: {current:?}")
            }
        };
        assert_eq!(renewed.token, acquired.token);
        assert!(renewed.lease_expires_at_ms >= acquired.lease_expires_at_ms);

        wrapper.conn.execute_batch("COMMIT").expect("commit tx");

        assert_eq!(business_event_count(&wrapper.conn), 1);
        let inspected = core::inspect(&wrapper.conn, "scheduler", system_now_for_core())
            .expect("core inspect after commit");
        assert_eq!(inspected, Some(renewed));
    }

    #[test]
    fn borrowed_claim_rolls_back_with_explicit_transaction() {
        let (_tempdir, wrapper) = open_wrapper_db();
        wrapper.bootstrap().expect("bootstrap");
        create_business_table(&wrapper.conn);
        let borrowed = wrapper.borrowed();

        wrapper.conn.execute_batch("BEGIN").expect("begin tx");
        wrapper
            .conn
            .execute(
                "INSERT INTO business_events(note) VALUES (?1)",
                params!["borrowed rollback"],
            )
            .expect("insert business event");

        let acquired = borrowed
            .claim("scheduler", "worker-a", Duration::from_secs(5))
            .expect("borrowed claim");
        assert!(matches!(acquired, ClaimResult::Acquired(_)));

        wrapper.conn.execute_batch("ROLLBACK").expect("rollback tx");

        assert_eq!(business_event_count(&wrapper.conn), 0);
        assert_eq!(
            core::inspect(&wrapper.conn, "scheduler", system_now_for_core())
                .expect("core inspect after rollback"),
            None
        );
        assert_eq!(
            core::token(&wrapper.conn, "scheduler").expect("core token after rollback"),
            None
        );
    }

    #[test]
    fn borrowed_multi_mutator_commit_together_inside_explicit_transaction() {
        let (_tempdir, wrapper) = open_wrapper_db();
        wrapper.bootstrap().expect("bootstrap");
        let borrowed = wrapper.borrowed();

        wrapper.conn.execute_batch("BEGIN").expect("begin tx");

        let scheduler = borrowed
            .claim("scheduler", "worker-a", Duration::from_secs(5))
            .expect("borrowed claim for scheduler");
        let janitor = borrowed
            .claim("janitor", "worker-b", Duration::from_secs(5))
            .expect("borrowed claim for janitor");

        assert!(matches!(scheduler, ClaimResult::Acquired(_)));
        assert!(matches!(janitor, ClaimResult::Acquired(_)));

        wrapper.conn.execute_batch("COMMIT").expect("commit tx");

        let now_ms = system_now_for_core();
        assert_eq!(
            core::owner(&wrapper.conn, "scheduler", now_ms).expect("scheduler owner"),
            Some("worker-a".to_owned())
        );
        assert_eq!(
            core::owner(&wrapper.conn, "janitor", now_ms).expect("janitor owner"),
            Some("worker-b".to_owned())
        );
    }

    #[test]
    fn borrowed_multi_mutator_rollback_together_inside_explicit_transaction() {
        let (_tempdir, wrapper) = open_wrapper_db();
        wrapper.bootstrap().expect("bootstrap");
        let borrowed = wrapper.borrowed();

        wrapper.conn.execute_batch("BEGIN").expect("begin tx");

        let scheduler = borrowed
            .claim("scheduler", "worker-a", Duration::from_secs(5))
            .expect("borrowed claim for scheduler");
        let janitor = borrowed
            .claim("janitor", "worker-b", Duration::from_secs(5))
            .expect("borrowed claim for janitor");

        assert!(matches!(scheduler, ClaimResult::Acquired(_)));
        assert!(matches!(janitor, ClaimResult::Acquired(_)));

        wrapper.conn.execute_batch("ROLLBACK").expect("rollback tx");

        let now_ms = system_now_for_core();
        assert_eq!(
            core::owner(&wrapper.conn, "scheduler", now_ms).expect("scheduler owner"),
            None
        );
        assert_eq!(
            core::owner(&wrapper.conn, "janitor", now_ms).expect("janitor owner"),
            None
        );
        assert_eq!(
            core::token(&wrapper.conn, "scheduler").expect("scheduler token"),
            None
        );
        assert_eq!(
            core::token(&wrapper.conn, "janitor").expect("janitor token"),
            None
        );
    }

    #[test]
    fn borrowed_mutators_preserve_lease_semantics_inside_explicit_transaction() {
        let (_tempdir, wrapper) = open_wrapper_db();
        wrapper.bootstrap().expect("bootstrap");
        let borrowed = wrapper.borrowed();

        wrapper.conn.execute_batch("BEGIN").expect("begin tx");

        let first_claim = borrowed
            .claim("scheduler", "worker-a", Duration::from_millis(20))
            .expect("first borrowed claim");
        let first_claim = match first_claim {
            ClaimResult::Acquired(lease) => lease,
            ClaimResult::Busy(current) => panic!("unexpected busy lease: {current:?}"),
        };

        let busy_claim = borrowed
            .claim("scheduler", "worker-b", Duration::from_millis(20))
            .expect("busy borrowed claim");
        match busy_claim {
            ClaimResult::Busy(current) => {
                assert_eq!(current.owner, "worker-a");
                assert_eq!(current.token, first_claim.token);
            }
            ClaimResult::Acquired(lease) => panic!("unexpected acquired lease: {lease:?}"),
        }

        std::thread::sleep(Duration::from_millis(30));

        let takeover = borrowed
            .claim("scheduler", "worker-b", Duration::from_millis(20))
            .expect("takeover claim");
        let takeover = match takeover {
            ClaimResult::Acquired(lease) => lease,
            ClaimResult::Busy(current) => panic!("unexpected busy lease: {current:?}"),
        };
        assert_eq!(takeover.token, first_claim.token + 1);

        let released = borrowed
            .release("scheduler", "worker-b")
            .expect("borrowed release");
        assert_eq!(
            released,
            ReleaseResult::Released {
                name: "scheduler".to_owned(),
                token: takeover.token,
            }
        );

        let reclaimed = borrowed
            .claim("scheduler", "worker-c", Duration::from_millis(200))
            .expect("reclaim claim");
        let reclaimed = match reclaimed {
            ClaimResult::Acquired(lease) => lease,
            ClaimResult::Busy(current) => panic!("unexpected busy lease: {current:?}"),
        };
        assert_eq!(reclaimed.token, takeover.token + 1);

        wrapper.conn.execute_batch("COMMIT").expect("commit tx");

        let inspected = core::inspect(&wrapper.conn, "scheduler", system_now_for_core())
            .expect("core inspect after semantic stress");
        assert_eq!(inspected, Some(reclaimed.clone()));
        assert_eq!(
            core::token(&wrapper.conn, "scheduler").expect("token after semantic stress"),
            Some(reclaimed.token)
        );
    }

    #[test]
    fn borrowed_mutators_work_inside_savepoint_context() {
        let (_tempdir, wrapper) = open_wrapper_db();
        wrapper.bootstrap().expect("bootstrap");
        let borrowed = wrapper.borrowed();

        wrapper
            .conn
            .execute_batch("SAVEPOINT borrowed_ops")
            .expect("begin savepoint");

        let acquired = borrowed
            .claim("scheduler", "worker-a", Duration::from_secs(5))
            .expect("borrowed claim inside savepoint");
        assert!(matches!(acquired, ClaimResult::Acquired(_)));

        wrapper
            .conn
            .execute_batch("ROLLBACK TO borrowed_ops")
            .expect("rollback to savepoint");
        wrapper
            .conn
            .execute_batch("RELEASE borrowed_ops")
            .expect("release savepoint");

        assert_eq!(
            core::owner(&wrapper.conn, "scheduler", system_now_for_core())
                .expect("owner after savepoint"),
            None
        );
    }

    #[test]
    fn transaction_handle_commits_business_write_and_lease_mutation() {
        let (_tempdir, mut wrapper) = open_wrapper_db();
        wrapper.bootstrap().expect("bootstrap");
        create_business_table(&wrapper.conn);

        let tx = wrapper.transaction().expect("begin immediate tx");
        tx.conn()
            .execute(
                "INSERT INTO business_events(note) VALUES (?1)",
                params!["tx commit"],
            )
            .expect("insert business event");

        let acquired = tx
            .claim("scheduler", "worker-a", Duration::from_secs(5))
            .expect("tx claim");
        let acquired = match acquired {
            ClaimResult::Acquired(lease) => lease,
            ClaimResult::Busy(current) => panic!("unexpected busy lease: {current:?}"),
        };

        tx.commit().expect("commit tx");

        assert_eq!(business_event_count(&wrapper.conn), 1);
        assert_eq!(
            core::inspect(&wrapper.conn, "scheduler", system_now_for_core())
                .expect("inspect after tx commit"),
            Some(acquired)
        );
    }

    #[test]
    fn transaction_handle_rolls_back_business_write_and_lease_mutation() {
        let (_tempdir, mut wrapper) = open_wrapper_db();
        wrapper.bootstrap().expect("bootstrap");
        create_business_table(&wrapper.conn);

        let tx = wrapper.transaction().expect("begin immediate tx");
        tx.conn()
            .execute(
                "INSERT INTO business_events(note) VALUES (?1)",
                params!["tx rollback"],
            )
            .expect("insert business event");
        let acquired = tx
            .claim("scheduler", "worker-a", Duration::from_secs(5))
            .expect("tx claim");
        assert!(matches!(acquired, ClaimResult::Acquired(_)));

        tx.rollback().expect("rollback tx");

        assert_eq!(business_event_count(&wrapper.conn), 0);
        assert_eq!(
            core::inspect(&wrapper.conn, "scheduler", system_now_for_core())
                .expect("inspect after tx rollback"),
            None
        );
        assert_eq!(
            core::token(&wrapper.conn, "scheduler").expect("token after tx rollback"),
            None
        );
    }

    #[test]
    fn transaction_handle_multiple_mutators_commit_together() {
        let (_tempdir, mut wrapper) = open_wrapper_db();
        wrapper.bootstrap().expect("bootstrap");

        let tx = wrapper.transaction().expect("begin immediate tx");

        let scheduler = tx
            .claim("scheduler", "worker-a", Duration::from_secs(5))
            .expect("tx claim scheduler");
        let janitor = tx
            .claim("janitor", "worker-b", Duration::from_secs(5))
            .expect("tx claim janitor");

        assert!(matches!(scheduler, ClaimResult::Acquired(_)));
        assert!(matches!(janitor, ClaimResult::Acquired(_)));

        tx.commit().expect("commit tx");

        let now_ms = system_now_for_core();
        assert_eq!(
            core::owner(&wrapper.conn, "scheduler", now_ms).expect("scheduler owner after commit"),
            Some("worker-a".to_owned())
        );
        assert_eq!(
            core::owner(&wrapper.conn, "janitor", now_ms).expect("janitor owner after commit"),
            Some("worker-b".to_owned())
        );
    }

    #[test]
    fn transaction_handle_multiple_mutators_rollback_together() {
        let (_tempdir, mut wrapper) = open_wrapper_db();
        wrapper.bootstrap().expect("bootstrap");

        let tx = wrapper.transaction().expect("begin immediate tx");

        let scheduler = tx
            .claim("scheduler", "worker-a", Duration::from_secs(5))
            .expect("tx claim scheduler");
        let janitor = tx
            .claim("janitor", "worker-b", Duration::from_secs(5))
            .expect("tx claim janitor");

        assert!(matches!(scheduler, ClaimResult::Acquired(_)));
        assert!(matches!(janitor, ClaimResult::Acquired(_)));

        tx.rollback().expect("rollback tx");

        let now_ms = system_now_for_core();
        assert_eq!(
            core::owner(&wrapper.conn, "scheduler", now_ms)
                .expect("scheduler owner after rollback"),
            None
        );
        assert_eq!(
            core::owner(&wrapper.conn, "janitor", now_ms).expect("janitor owner after rollback"),
            None
        );
        assert_eq!(
            core::token(&wrapper.conn, "scheduler").expect("scheduler token after rollback"),
            None
        );
        assert_eq!(
            core::token(&wrapper.conn, "janitor").expect("janitor token after rollback"),
            None
        );
    }

    #[test]
    fn dropping_transaction_handle_without_commit_rolls_back() {
        let (_tempdir, mut wrapper) = open_wrapper_db();
        wrapper.bootstrap().expect("bootstrap");
        create_business_table(&wrapper.conn);

        {
            let tx = wrapper.transaction().expect("begin immediate tx");
            tx.conn()
                .execute(
                    "INSERT INTO business_events(note) VALUES (?1)",
                    params!["tx drop rollback"],
                )
                .expect("insert business event");
            let acquired = tx
                .claim("scheduler", "worker-a", Duration::from_secs(5))
                .expect("tx claim");
            assert!(matches!(acquired, ClaimResult::Acquired(_)));
        }

        assert_eq!(business_event_count(&wrapper.conn), 0);
        assert_eq!(
            core::inspect(&wrapper.conn, "scheduler", system_now_for_core())
                .expect("inspect after dropped tx"),
            None
        );
    }

    #[test]
    fn transaction_handle_preserves_lease_semantics() {
        let (_tempdir, mut wrapper) = open_wrapper_db();
        wrapper.bootstrap().expect("bootstrap");

        let tx = wrapper.transaction().expect("begin immediate tx");

        let first_claim = tx
            .claim("scheduler", "worker-a", Duration::from_millis(20))
            .expect("first tx claim");
        let first_claim = match first_claim {
            ClaimResult::Acquired(lease) => lease,
            ClaimResult::Busy(current) => panic!("unexpected busy lease: {current:?}"),
        };

        let busy_claim = tx
            .claim("scheduler", "worker-b", Duration::from_millis(20))
            .expect("busy tx claim");
        match busy_claim {
            ClaimResult::Busy(current) => {
                assert_eq!(current.owner, "worker-a");
                assert_eq!(current.token, first_claim.token);
            }
            ClaimResult::Acquired(lease) => panic!("unexpected acquired lease: {lease:?}"),
        }

        std::thread::sleep(Duration::from_millis(30));

        let takeover = tx
            .claim("scheduler", "worker-b", Duration::from_millis(20))
            .expect("takeover tx claim");
        let takeover = match takeover {
            ClaimResult::Acquired(lease) => lease,
            ClaimResult::Busy(current) => panic!("unexpected busy lease: {current:?}"),
        };
        assert_eq!(takeover.token, first_claim.token + 1);

        let released = tx.release("scheduler", "worker-b").expect("tx release");
        assert_eq!(
            released,
            ReleaseResult::Released {
                name: "scheduler".to_owned(),
                token: takeover.token,
            }
        );

        let reclaimed = tx
            .claim("scheduler", "worker-c", Duration::from_millis(200))
            .expect("reclaim tx claim");
        let reclaimed = match reclaimed {
            ClaimResult::Acquired(lease) => lease,
            ClaimResult::Busy(current) => panic!("unexpected busy lease: {current:?}"),
        };
        assert_eq!(reclaimed.token, takeover.token + 1);

        tx.commit().expect("commit tx");

        let inspected = core::inspect(&wrapper.conn, "scheduler", system_now_for_core())
            .expect("inspect after tx semantic stress");
        assert_eq!(inspected, Some(reclaimed.clone()));
        assert_eq!(
            core::token(&wrapper.conn, "scheduler").expect("token after tx semantic stress"),
            Some(reclaimed.token)
        );
    }

    #[test]
    fn transaction_handle_renew_extends_existing_lease() {
        let (_tempdir, mut wrapper) = open_wrapper_db();
        wrapper.bootstrap().expect("bootstrap");

        let tx = wrapper.transaction().expect("begin immediate tx");

        let acquired = tx
            .claim("scheduler", "worker-a", Duration::from_secs(5))
            .expect("tx claim");
        let acquired = match acquired {
            ClaimResult::Acquired(lease) => lease,
            ClaimResult::Busy(current) => panic!("unexpected busy lease: {current:?}"),
        };

        let renewed = tx
            .renew("scheduler", "worker-a", Duration::from_secs(60))
            .expect("tx renew owner-match");
        let renewed = match renewed {
            RenewResult::Renewed(lease) => lease,
            RenewResult::Rejected { current } => {
                panic!("unexpected renew rejection: {current:?}")
            }
        };
        assert_eq!(renewed.token, acquired.token);
        assert_eq!(renewed.owner, acquired.owner);
        assert!(renewed.lease_expires_at_ms >= acquired.lease_expires_at_ms);

        let wrong_owner = tx
            .renew("scheduler", "worker-b", Duration::from_secs(60))
            .expect("tx renew wrong-owner");
        match wrong_owner {
            RenewResult::Rejected {
                current: Some(current),
            } => {
                assert_eq!(current.owner, "worker-a");
                assert_eq!(current.token, renewed.token);
            }
            other => panic!("unexpected wrong-owner renew result: {other:?}"),
        }

        let missing = tx
            .renew("never-claimed", "worker-a", Duration::from_secs(60))
            .expect("tx renew missing-resource");
        assert_eq!(missing, RenewResult::Rejected { current: None });

        tx.commit().expect("commit tx");

        let inspected = core::inspect(&wrapper.conn, "scheduler", system_now_for_core())
            .expect("inspect after tx renew commit");
        assert_eq!(inspected, Some(renewed));
    }

    #[test]
    fn transaction_handle_inspect_returns_live_lease() {
        let (_tempdir, mut wrapper) = open_wrapper_db();
        wrapper.bootstrap().expect("bootstrap");

        let tx = wrapper.transaction().expect("begin immediate tx");

        assert_eq!(
            tx.inspect("scheduler").expect("tx inspect before claim"),
            None
        );

        let acquired = tx
            .claim("scheduler", "worker-a", Duration::from_secs(5))
            .expect("tx claim");
        let acquired = match acquired {
            ClaimResult::Acquired(lease) => lease,
            ClaimResult::Busy(current) => panic!("unexpected busy lease: {current:?}"),
        };

        let inspected = tx
            .inspect("scheduler")
            .expect("tx inspect after claim")
            .expect("expected live lease inside tx");
        assert_eq!(inspected, acquired);

        assert_eq!(
            tx.inspect("never-claimed")
                .expect("tx inspect missing resource"),
            None
        );

        tx.commit().expect("commit tx");
    }
}
