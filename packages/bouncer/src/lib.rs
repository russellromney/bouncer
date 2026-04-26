use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use bouncer_honker as core;
use rusqlite::Connection;

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
        core::claim(
            self.conn,
            name,
            owner,
            system_now_ms()?,
            duration_to_ttl_ms(ttl)?,
        )
        .map_err(Error::from)
    }

    pub fn renew(&self, name: &str, owner: &str, ttl: Duration) -> Result<RenewResult> {
        core::renew(
            self.conn,
            name,
            owner,
            system_now_ms()?,
            duration_to_ttl_ms(ttl)?,
        )
        .map_err(Error::from)
    }

    pub fn release(&self, name: &str, owner: &str) -> Result<ReleaseResult> {
        core::release(self.conn, name, owner, system_now_ms()?).map_err(Error::from)
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
}
