//! Core lease state machine and SQLite operations for Bouncer.
//!
//! Use `claim`, `renew`, and `release` when this crate should open and
//! commit its own `BEGIN IMMEDIATE` transaction for the mutation.
//! Use `claim_in_tx`, `renew_in_tx`, and `release_in_tx` when the caller
//! already owns the surrounding transaction or savepoint and wants Bouncer
//! to participate in that existing atomic boundary.
//! The two surfaces share the same lease semantics; they differ only in who
//! owns transaction opening, commit, and lock-upgrade timing.
//!
use rusqlite::functions::FunctionFlags;
use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};

pub const BOUNCER_SCHEMA_VERSION: &str = "1";

pub type Result<T> = std::result::Result<T, Error>;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("database error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("connection must already be inside a transaction or savepoint")]
    NotInTransaction,
    #[error("ttl_ms must be positive, got {0}")]
    InvalidTtlMs(i64),
    #[error("ttl_ms {ttl_ms} at now_ms {now_ms} overflows lease expiry")]
    TtlOverflow { now_ms: i64, ttl_ms: i64 },
    #[error("fencing token overflow for resource `{0}`")]
    TokenOverflow(String),
    #[error("invalid lease row for resource `{0}`")]
    InvalidLeaseRow(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeaseInfo {
    pub name: String,
    pub owner: String,
    pub token: i64,
    pub lease_expires_at_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClaimResult {
    Acquired(LeaseInfo),
    Busy(LeaseInfo),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenewResult {
    Renewed(LeaseInfo),
    Rejected { current: Option<LeaseInfo> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReleaseResult {
    Released { name: String, token: i64 },
    Rejected { current: Option<LeaseInfo> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ResourceRow {
    name: String,
    owner: Option<String>,
    token: i64,
    lease_expires_at_ms: Option<i64>,
    created_at_ms: i64,
    updated_at_ms: i64,
}

impl ResourceRow {
    fn current_lease(&self, now_ms: i64) -> Result<Option<LeaseInfo>> {
        self.validate()?;

        match (&self.owner, self.lease_expires_at_ms) {
            (Some(owner), Some(lease_expires_at_ms)) if lease_expires_at_ms > now_ms => {
                Ok(Some(LeaseInfo {
                    name: self.name.clone(),
                    owner: owner.clone(),
                    token: self.token,
                    lease_expires_at_ms,
                }))
            }
            _ => Ok(None),
        }
    }

    fn validate(&self) -> Result<()> {
        let pair_is_valid = matches!(
            (&self.owner, self.lease_expires_at_ms),
            (Some(_), Some(_)) | (None, None)
        );

        if !pair_is_valid || self.token <= 0 {
            return Err(Error::InvalidLeaseRow(self.name.clone()));
        }

        Ok(())
    }
}

/// Install Bouncer's Phase 001 schema on `conn`. Idempotent.
pub fn bootstrap_bouncer_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS bouncer_resources (
           name TEXT PRIMARY KEY,
           owner TEXT,
           token INTEGER NOT NULL CHECK (token >= 1),
           lease_expires_at_ms INTEGER,
           created_at_ms INTEGER NOT NULL,
           updated_at_ms INTEGER NOT NULL,
           CHECK (
             (owner IS NULL AND lease_expires_at_ms IS NULL)
             OR (owner IS NOT NULL AND lease_expires_at_ms IS NOT NULL)
           )
         );",
    )?;

    Ok(())
}

/// Return the live lease for `name` at `now_ms`, if one exists.
pub fn inspect(conn: &Connection, name: &str, now_ms: i64) -> Result<Option<LeaseInfo>> {
    load_resource(conn, name)?
        .map(|row| row.current_lease(now_ms))
        .transpose()
        .map(Option::flatten)
}

/// Return the current live owner for `name` at `now_ms`, if one exists.
pub fn owner(conn: &Connection, name: &str, now_ms: i64) -> Result<Option<String>> {
    Ok(inspect(conn, name, now_ms)?.map(|lease| lease.owner))
}

/// Return the current fencing token for `name`, even if no live lease exists.
pub fn token(conn: &Connection, name: &str) -> Result<Option<i64>> {
    Ok(load_resource(conn, name)?.map(|row| row.token))
}

/// Attempt to claim a named resource for `owner`.
pub fn claim(
    conn: &Connection,
    name: &str,
    owner: &str,
    now_ms: i64,
    ttl_ms: i64,
) -> Result<ClaimResult> {
    let tx = begin_immediate(conn)?;
    let result = claim_in_tx(&tx, name, owner, now_ms, ttl_ms)?;

    tx.commit()?;
    Ok(result)
}

/// Renew a currently-live lease owned by `owner`.
pub fn renew(
    conn: &Connection,
    name: &str,
    owner: &str,
    now_ms: i64,
    ttl_ms: i64,
) -> Result<RenewResult> {
    let tx = begin_immediate(conn)?;
    let result = renew_in_tx(&tx, name, owner, now_ms, ttl_ms)?;

    tx.commit()?;
    Ok(result)
}

/// Release a currently-live lease owned by `owner`.
pub fn release(conn: &Connection, name: &str, owner: &str, now_ms: i64) -> Result<ReleaseResult> {
    let tx = begin_immediate(conn)?;
    let result = release_in_tx(&tx, name, owner, now_ms)?;

    tx.commit()?;
    Ok(result)
}

/// Attempt to claim a resource using the caller's current transaction.
///
/// `conn` must already be inside the transaction or savepoint that
/// should own atomicity for this mutation, which is equivalent to
/// `conn.is_autocommit() == false`. If that precondition is violated,
/// this returns [`Error::NotInTransaction`] instead of silently running
/// outside a caller-owned transaction.
///
/// This helper does not open or commit a transaction. Lock-upgrade
/// timing follows the caller's outer transaction mode.
pub fn claim_in_tx(
    conn: &Connection,
    name: &str,
    owner: &str,
    now_ms: i64,
    ttl_ms: i64,
) -> Result<ClaimResult> {
    ensure_in_tx(conn)?;
    let lease_expires_at_ms = checked_expiry(now_ms, ttl_ms)?;

    match load_resource(conn, name)? {
        None => {
            conn.execute(
                "INSERT INTO bouncer_resources (
                   name,
                   owner,
                   token,
                   lease_expires_at_ms,
                   created_at_ms,
                   updated_at_ms
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![name, owner, 1_i64, lease_expires_at_ms, now_ms, now_ms],
            )?;

            Ok(ClaimResult::Acquired(LeaseInfo {
                name: name.to_owned(),
                owner: owner.to_owned(),
                token: 1,
                lease_expires_at_ms,
            }))
        }
        Some(row) => match row.current_lease(now_ms)? {
            Some(current) => Ok(ClaimResult::Busy(current)),
            None => {
                let next_token = row
                    .token
                    .checked_add(1)
                    .ok_or_else(|| Error::TokenOverflow(row.name.clone()))?;

                conn.execute(
                    "UPDATE bouncer_resources
                     SET owner = ?2,
                         token = ?3,
                         lease_expires_at_ms = ?4,
                         updated_at_ms = ?5
                     WHERE name = ?1",
                    params![name, owner, next_token, lease_expires_at_ms, now_ms],
                )?;

                Ok(ClaimResult::Acquired(LeaseInfo {
                    name: row.name,
                    owner: owner.to_owned(),
                    token: next_token,
                    lease_expires_at_ms,
                }))
            }
        },
    }
}

/// Renew a lease using the caller's current transaction.
///
/// `conn` must already be inside the transaction or savepoint that
/// should own atomicity for this mutation, which is equivalent to
/// `conn.is_autocommit() == false`. If that precondition is violated,
/// this returns [`Error::NotInTransaction`] instead of silently running
/// outside a caller-owned transaction.
///
/// This helper does not open or commit a transaction. Lock-upgrade
/// timing follows the caller's outer transaction mode.
pub fn renew_in_tx(
    conn: &Connection,
    name: &str,
    owner: &str,
    now_ms: i64,
    ttl_ms: i64,
) -> Result<RenewResult> {
    ensure_in_tx(conn)?;
    let requested_expires_at_ms = checked_expiry(now_ms, ttl_ms)?;

    match load_resource(conn, name)? {
        None => Ok(RenewResult::Rejected { current: None }),
        Some(row) => match row.current_lease(now_ms)? {
            Some(current) if current.owner == owner => {
                let lease_expires_at_ms =
                    current.lease_expires_at_ms.max(requested_expires_at_ms);

                conn.execute(
                    "UPDATE bouncer_resources
                     SET lease_expires_at_ms = ?2,
                         updated_at_ms = ?3
                     WHERE name = ?1",
                    params![name, lease_expires_at_ms, now_ms],
                )?;

                Ok(RenewResult::Renewed(LeaseInfo {
                    lease_expires_at_ms,
                    ..current
                }))
            }
            Some(current) => Ok(RenewResult::Rejected {
                current: Some(current),
            }),
            None => Ok(RenewResult::Rejected { current: None }),
        },
    }
}

/// Release a lease using the caller's current transaction.
///
/// `conn` must already be inside the transaction or savepoint that
/// should own atomicity for this mutation, which is equivalent to
/// `conn.is_autocommit() == false`. If that precondition is violated,
/// this returns [`Error::NotInTransaction`] instead of silently running
/// outside a caller-owned transaction.
///
/// This helper does not open or commit a transaction. Lock-upgrade
/// timing follows the caller's outer transaction mode.
pub fn release_in_tx(
    conn: &Connection,
    name: &str,
    owner: &str,
    now_ms: i64,
) -> Result<ReleaseResult> {
    ensure_in_tx(conn)?;
    match load_resource(conn, name)? {
        None => Ok(ReleaseResult::Rejected { current: None }),
        Some(row) => match row.current_lease(now_ms)? {
            Some(current) if current.owner == owner => {
                conn.execute(
                    "UPDATE bouncer_resources
                     SET owner = NULL,
                         lease_expires_at_ms = NULL,
                         updated_at_ms = ?2
                     WHERE name = ?1",
                    params![name, now_ms],
                )?;

                Ok(ReleaseResult::Released {
                    name: current.name,
                    token: current.token,
                })
            }
            Some(current) => Ok(ReleaseResult::Rejected {
                current: Some(current),
            }),
            None => Ok(ReleaseResult::Rejected { current: None }),
        },
    }
}

/// Register all `bouncer_*` scalar functions on `conn`.
///
/// Mutating SQL helpers (`claim`, `renew`, `release`) use one of two paths:
///
/// - in autocommit mode, they delegate to the same public core helpers as
///   direct Rust callers, which open `BEGIN IMMEDIATE`
/// - inside an already-open transaction or savepoint, they reuse the current
///   connection state through the `*_in_tx` helpers instead of attempting a
///   nested transaction
///
/// The second path means lock-upgrade timing follows the caller's outer
/// transaction mode. A caller that wants the old up-front writer claim inside
/// a transaction should begin that outer transaction with `BEGIN IMMEDIATE`.
///
/// `ctx.get_connection()` is still the right seam here because these SQL
/// helpers intentionally operate on the caller's current connection and
/// transaction context rather than opening a sibling handle.
pub fn attach_bouncer_functions(conn: &Connection) -> rusqlite::Result<()> {
    conn.create_scalar_function("bouncer_bootstrap", 0, FunctionFlags::SQLITE_UTF8, |ctx| {
        let db = unsafe { ctx.get_connection() }?;
        bootstrap_bouncer_schema(&db).map_err(to_sql_err)?;
        Ok(1i64)
    })?;

    conn.create_scalar_function("bouncer_claim", 4, FunctionFlags::SQLITE_UTF8, |ctx| {
        let name: String = ctx.get(0)?;
        let owner: String = ctx.get(1)?;
        let ttl_ms: i64 = ctx.get(2)?;
        let now_ms: i64 = ctx.get(3)?;
        let db = unsafe { ctx.get_connection() }?;

        let result = if db.is_autocommit() {
            claim(&db, &name, &owner, now_ms, ttl_ms)
        } else {
            claim_in_tx(&db, &name, &owner, now_ms, ttl_ms)
        }
        .map_err(to_sql_err)?;

        match result {
            ClaimResult::Acquired(lease) => Ok(Some(lease.token)),
            ClaimResult::Busy(_) => Ok(None),
        }
    })?;

    conn.create_scalar_function("bouncer_renew", 4, FunctionFlags::SQLITE_UTF8, |ctx| {
        let name: String = ctx.get(0)?;
        let owner: String = ctx.get(1)?;
        let ttl_ms: i64 = ctx.get(2)?;
        let now_ms: i64 = ctx.get(3)?;
        let db = unsafe { ctx.get_connection() }?;

        let result = if db.is_autocommit() {
            renew(&db, &name, &owner, now_ms, ttl_ms)
        } else {
            renew_in_tx(&db, &name, &owner, now_ms, ttl_ms)
        }
        .map_err(to_sql_err)?;

        match result {
            RenewResult::Renewed(lease) => Ok(Some(lease.token)),
            RenewResult::Rejected { .. } => Ok(None),
        }
    })?;

    conn.create_scalar_function("bouncer_release", 3, FunctionFlags::SQLITE_UTF8, |ctx| {
        let name: String = ctx.get(0)?;
        let owner: String = ctx.get(1)?;
        let now_ms: i64 = ctx.get(2)?;
        let db = unsafe { ctx.get_connection() }?;

        let result = if db.is_autocommit() {
            release(&db, &name, &owner, now_ms)
        } else {
            release_in_tx(&db, &name, &owner, now_ms)
        }
        .map_err(to_sql_err)?;

        match result {
            ReleaseResult::Released { .. } => Ok(1i64),
            ReleaseResult::Rejected { .. } => Ok(0i64),
        }
    })?;

    conn.create_scalar_function("bouncer_owner", 2, FunctionFlags::SQLITE_UTF8, |ctx| {
        let name: String = ctx.get(0)?;
        let now_ms: i64 = ctx.get(1)?;
        let db = unsafe { ctx.get_connection() }?;
        owner(&db, &name, now_ms).map_err(to_sql_err)
    })?;

    conn.create_scalar_function("bouncer_token", 1, FunctionFlags::SQLITE_UTF8, |ctx| {
        let name: String = ctx.get(0)?;
        let db = unsafe { ctx.get_connection() }?;
        token(&db, &name).map_err(to_sql_err)
    })?;

    Ok(())
}

fn begin_immediate(conn: &Connection) -> rusqlite::Result<Transaction<'_>> {
    Transaction::new_unchecked(conn, TransactionBehavior::Immediate)
}

fn ensure_in_tx(conn: &Connection) -> Result<()> {
    if conn.is_autocommit() {
        return Err(Error::NotInTransaction);
    }

    Ok(())
}

fn to_sql_err(err: Error) -> rusqlite::Error {
    match err {
        Error::Sqlite(err) => err,
        other => {
            rusqlite::Error::UserFunctionError(Box::new(std::io::Error::other(other.to_string())))
        }
    }
}

fn checked_expiry(now_ms: i64, ttl_ms: i64) -> Result<i64> {
    if ttl_ms <= 0 {
        return Err(Error::InvalidTtlMs(ttl_ms));
    }

    now_ms
        .checked_add(ttl_ms)
        .ok_or(Error::TtlOverflow { now_ms, ttl_ms })
}

fn load_resource(conn: &Connection, name: &str) -> Result<Option<ResourceRow>> {
    let row = conn
        .query_row(
            "SELECT
               name,
               owner,
               token,
               lease_expires_at_ms,
               created_at_ms,
               updated_at_ms
             FROM bouncer_resources
             WHERE name = ?1",
            params![name],
            |row| {
                Ok(ResourceRow {
                    name: row.get(0)?,
                    owner: row.get(1)?,
                    token: row.get(2)?,
                    lease_expires_at_ms: row.get(3)?,
                    created_at_ms: row.get(4)?,
                    updated_at_ms: row.get(5)?,
                })
            },
        )
        .optional()?;

    match row {
        Some(row) => {
            row.validate()?;
            Ok(Some(row))
        }
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tempfile::TempDir;

    fn open_db() -> Connection {
        let conn = Connection::open_in_memory().expect("in-memory sqlite");
        bootstrap_bouncer_schema(&conn).expect("bootstrap schema");
        conn
    }

    fn open_shared_db() -> (TempDir, Connection, Connection) {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let db_path = tempdir.path().join("bouncer.sqlite3");

        let conn_a = Connection::open(&db_path).expect("open first sqlite connection");
        bootstrap_bouncer_schema(&conn_a).expect("bootstrap schema on first connection");

        let conn_b = Connection::open(&db_path).expect("open second sqlite connection");
        bootstrap_bouncer_schema(&conn_b).expect("bootstrap schema on second connection");

        (tempdir, conn_a, conn_b)
    }

    fn open_sql_db() -> Connection {
        let conn = Connection::open_in_memory().expect("in-memory sqlite");
        attach_bouncer_functions(&conn).expect("attach bouncer sql functions");
        conn
    }

    fn open_shared_sql_db() -> (TempDir, Connection, Connection) {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let db_path = tempdir.path().join("bouncer.sqlite3");

        let conn_a = Connection::open(&db_path).expect("open first sql connection");
        conn_a
            .busy_timeout(Duration::from_millis(0))
            .expect("set zero busy timeout on first sql connection");
        attach_bouncer_functions(&conn_a).expect("attach functions to first sql connection");
        bootstrap_bouncer_schema(&conn_a).expect("bootstrap schema on first sql connection");

        let conn_b = Connection::open(&db_path).expect("open second sql connection");
        conn_b
            .busy_timeout(Duration::from_millis(0))
            .expect("set zero busy timeout on second sql connection");
        attach_bouncer_functions(&conn_b).expect("attach functions to second sql connection");
        bootstrap_bouncer_schema(&conn_b).expect("bootstrap schema on second sql connection");

        (tempdir, conn_a, conn_b)
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

    fn assert_live_lease(
        lease: &LeaseInfo,
        name: &str,
        owner: &str,
        token: i64,
        lease_expires_at_ms: i64,
    ) {
        assert_eq!(
            lease,
            &LeaseInfo {
                name: name.to_owned(),
                owner: owner.to_owned(),
                token,
                lease_expires_at_ms,
            }
        );
    }

    #[test]
    fn bootstrap_is_idempotent() {
        let conn = Connection::open_in_memory().unwrap();

        bootstrap_bouncer_schema(&conn).unwrap();
        bootstrap_bouncer_schema(&conn).unwrap();

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master
                 WHERE type = 'table' AND name = 'bouncer_resources'",
                [],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(count, 1);
    }

    #[test]
    fn inspect_returns_none_for_absent_resource() {
        let conn = open_db();

        assert_eq!(inspect(&conn, "scheduler", 100).unwrap(), None);
        assert_eq!(owner(&conn, "scheduler", 100).unwrap(), None);
        assert_eq!(token(&conn, "scheduler").unwrap(), None);
    }

    #[test]
    fn inspect_returns_none_for_expired_resource() {
        let conn = open_db();

        let result = claim(&conn, "scheduler", "worker-a", 100, 25).unwrap();
        assert!(matches!(result, ClaimResult::Acquired(_)));

        assert_eq!(inspect(&conn, "scheduler", 125).unwrap(), None);
        assert_eq!(inspect(&conn, "scheduler", 500).unwrap(), None);
    }

    #[test]
    fn inspect_returns_none_for_released_resource() {
        let conn = open_db();

        release_after_claim(&conn);

        assert_eq!(inspect(&conn, "scheduler", 200).unwrap(), None);
    }

    #[test]
    fn first_claim_acquires_lease() {
        let conn = open_db();

        let result = claim(&conn, "scheduler", "worker-a", 100, 50).unwrap();

        match result {
            ClaimResult::Acquired(lease) => {
                assert_live_lease(&lease, "scheduler", "worker-a", 1, 150);
            }
            other => panic!("expected acquired result, got {other:?}"),
        }
    }

    #[test]
    fn second_claim_while_valid_is_rejected() {
        let conn = open_db();

        claim(&conn, "scheduler", "worker-a", 100, 50).unwrap();
        let result = claim(&conn, "scheduler", "worker-b", 120, 50).unwrap();

        match result {
            ClaimResult::Busy(current) => {
                assert_live_lease(&current, "scheduler", "worker-a", 1, 150);
            }
            other => panic!("expected busy result, got {other:?}"),
        }
    }

    #[test]
    fn expired_claim_takeover_succeeds_and_increments_token() {
        let conn = open_db();

        claim(&conn, "scheduler", "worker-a", 100, 50).unwrap();
        let result = claim(&conn, "scheduler", "worker-b", 151, 30).unwrap();

        match result {
            ClaimResult::Acquired(lease) => {
                assert_live_lease(&lease, "scheduler", "worker-b", 2, 181);
            }
            other => panic!("expected acquired result, got {other:?}"),
        }
    }

    #[test]
    fn released_resource_can_be_claimed_again_without_resetting_token() {
        let conn = open_db();

        release_after_claim(&conn);
        let result = claim(&conn, "scheduler", "worker-b", 220, 40).unwrap();

        match result {
            ClaimResult::Acquired(lease) => {
                assert_live_lease(&lease, "scheduler", "worker-b", 2, 260);
            }
            other => panic!("expected acquired result, got {other:?}"),
        }

        assert_eq!(token(&conn, "scheduler").unwrap(), Some(2));
    }

    #[test]
    fn renew_succeeds_for_current_owner() {
        let conn = open_db();

        claim(&conn, "scheduler", "worker-a", 100, 50).unwrap();
        let result = renew(&conn, "scheduler", "worker-a", 120, 60).unwrap();

        match result {
            RenewResult::Renewed(lease) => {
                assert_live_lease(&lease, "scheduler", "worker-a", 1, 180);
            }
            other => panic!("expected renewed result, got {other:?}"),
        }
    }

    #[test]
    fn renew_does_not_shorten_existing_lease() {
        let conn = open_db();

        claim(&conn, "scheduler", "worker-a", 100, 100).unwrap();
        let result = renew(&conn, "scheduler", "worker-a", 120, 10).unwrap();

        match result {
            RenewResult::Renewed(lease) => {
                assert_live_lease(&lease, "scheduler", "worker-a", 1, 200);
            }
            other => panic!("expected renewed result, got {other:?}"),
        }

        assert_live_lease(
            &inspect(&conn, "scheduler", 199)
                .unwrap()
                .expect("lease should remain live until original expiry"),
            "scheduler",
            "worker-a",
            1,
            200,
        );
    }

    #[test]
    fn renew_fails_for_non_owner() {
        let conn = open_db();

        claim(&conn, "scheduler", "worker-a", 100, 50).unwrap();
        let result = renew(&conn, "scheduler", "worker-b", 120, 60).unwrap();

        match result {
            RenewResult::Rejected {
                current: Some(current),
            } => {
                assert_live_lease(&current, "scheduler", "worker-a", 1, 150);
            }
            other => panic!("expected rejection with current lease, got {other:?}"),
        }
    }

    #[test]
    fn renew_fails_for_expired_lease() {
        let conn = open_db();

        claim(&conn, "scheduler", "worker-a", 100, 50).unwrap();
        let result = renew(&conn, "scheduler", "worker-a", 150, 60).unwrap();

        assert_eq!(result, RenewResult::Rejected { current: None });
    }

    #[test]
    fn release_succeeds_for_current_owner() {
        let conn = open_db();

        claim(&conn, "scheduler", "worker-a", 100, 50).unwrap();
        let result = release(&conn, "scheduler", "worker-a", 120).unwrap();

        assert_eq!(
            result,
            ReleaseResult::Released {
                name: "scheduler".to_owned(),
                token: 1,
            }
        );
    }

    #[test]
    fn release_fails_for_non_owner() {
        let conn = open_db();

        claim(&conn, "scheduler", "worker-a", 100, 50).unwrap();
        let result = release(&conn, "scheduler", "worker-b", 120).unwrap();

        match result {
            ReleaseResult::Rejected {
                current: Some(current),
            } => {
                assert_live_lease(&current, "scheduler", "worker-a", 1, 150);
            }
            other => panic!("expected rejection with current lease, got {other:?}"),
        }
    }

    #[test]
    fn non_positive_ttl_is_rejected() {
        let conn = open_db();

        let err = claim(&conn, "scheduler", "worker-a", 100, 0).unwrap_err();
        assert!(matches!(err, Error::InvalidTtlMs(0)));

        let err = renew(&conn, "scheduler", "worker-a", 100, -5).unwrap_err();
        assert!(matches!(err, Error::InvalidTtlMs(-5)));
    }

    #[test]
    fn in_tx_helpers_reject_autocommit_connections() {
        let conn = open_db();

        let err = claim_in_tx(&conn, "scheduler", "worker-a", 100, 50).unwrap_err();
        assert!(matches!(err, Error::NotInTransaction));

        let err = renew_in_tx(&conn, "scheduler", "worker-a", 100, 50).unwrap_err();
        assert!(matches!(err, Error::NotInTransaction));

        let err = release_in_tx(&conn, "scheduler", "worker-a", 100).unwrap_err();
        assert!(matches!(err, Error::NotInTransaction));
    }

    #[test]
    fn multiple_connections_share_live_lease_state() {
        let (_tempdir, conn_a, conn_b) = open_shared_db();

        let claimed = claim(&conn_a, "scheduler", "worker-a", 100, 50).unwrap();
        match claimed {
            ClaimResult::Acquired(lease) => {
                assert_live_lease(&lease, "scheduler", "worker-a", 1, 150);
            }
            other => panic!("expected acquired result, got {other:?}"),
        }

        let seen = inspect(&conn_b, "scheduler", 120).unwrap();
        match seen {
            Some(lease) => assert_live_lease(&lease, "scheduler", "worker-a", 1, 150),
            None => panic!("expected visible live lease from second connection"),
        }

        let busy = claim(&conn_b, "scheduler", "worker-b", 120, 30).unwrap();
        match busy {
            ClaimResult::Busy(current) => {
                assert_live_lease(&current, "scheduler", "worker-a", 1, 150);
            }
            other => panic!("expected busy result, got {other:?}"),
        }
    }

    #[test]
    fn multiple_connections_allow_expired_handoff() {
        let (_tempdir, conn_a, conn_b) = open_shared_db();

        claim(&conn_a, "scheduler", "worker-a", 100, 50).unwrap();

        let takeover = claim(&conn_b, "scheduler", "worker-b", 151, 30).unwrap();
        match takeover {
            ClaimResult::Acquired(lease) => {
                assert_live_lease(&lease, "scheduler", "worker-b", 2, 181);
            }
            other => panic!("expected acquired takeover result, got {other:?}"),
        }

        let seen = inspect(&conn_a, "scheduler", 160).unwrap();
        match seen {
            Some(lease) => assert_live_lease(&lease, "scheduler", "worker-b", 2, 181),
            None => panic!("expected takeover to be visible from first connection"),
        }
    }

    #[test]
    fn deferred_sql_transactions_surface_busy_under_writer_contention() {
        let (_tempdir, conn_a, conn_b) = open_shared_sql_db();

        conn_a
            .execute_batch("BEGIN")
            .expect("begin first deferred tx");
        conn_b
            .execute_batch("BEGIN")
            .expect("begin second deferred tx");

        let first_claim = conn_a
            .query_row(
                "SELECT bouncer_claim(?1, ?2, ?3, ?4)",
                params!["scheduler", "worker-a", 50_i64, 100_i64],
                |row| row.get::<_, Option<i64>>(0),
            )
            .expect("first in-tx claim should succeed");
        assert_eq!(first_claim, Some(1));

        let err = conn_b
            .query_row(
                "SELECT bouncer_claim(?1, ?2, ?3, ?4)",
                params!["scheduler", "worker-b", 50_i64, 100_i64],
                |row| row.get::<_, Option<i64>>(0),
            )
            .expect_err("second deferred writer should hit SQLITE_BUSY");
        let message = err.to_string();
        assert!(
            message.contains("database is locked") || message.contains("database is busy"),
            "expected lock/busy failure, got: {message}"
        );

        conn_b
            .execute_batch("ROLLBACK")
            .expect("rollback second tx");
        conn_a.execute_batch("ROLLBACK").expect("rollback first tx");
    }

    #[test]
    fn attached_sql_functions_cover_bootstrap_and_full_lease_cycle() {
        let conn = open_sql_db();

        let bootstrapped: i64 = conn
            .query_row("SELECT bouncer_bootstrap()", [], |row| row.get(0))
            .unwrap();
        assert_eq!(bootstrapped, 1);

        let first_claim: Option<i64> = conn
            .query_row(
                "SELECT bouncer_claim(?1, ?2, ?3, ?4)",
                params!["scheduler", "worker-a", 50_i64, 100_i64],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(first_claim, Some(1));

        let owner_after_claim: Option<String> = conn
            .query_row(
                "SELECT bouncer_owner(?1, ?2)",
                params!["scheduler", 120_i64],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(owner_after_claim.as_deref(), Some("worker-a"));

        let token_after_claim: Option<i64> = conn
            .query_row("SELECT bouncer_token(?1)", params!["scheduler"], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(token_after_claim, Some(1));

        let renewed: Option<i64> = conn
            .query_row(
                "SELECT bouncer_renew(?1, ?2, ?3, ?4)",
                params!["scheduler", "worker-a", 60_i64, 120_i64],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(renewed, Some(1));

        let released: i64 = conn
            .query_row(
                "SELECT bouncer_release(?1, ?2, ?3)",
                params!["scheduler", "worker-a", 140_i64],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(released, 1);

        let owner_after_release: Option<String> = conn
            .query_row(
                "SELECT bouncer_owner(?1, ?2)",
                params!["scheduler", 141_i64],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(owner_after_release, None);

        let token_after_release: Option<i64> = conn
            .query_row("SELECT bouncer_token(?1)", params!["scheduler"], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(token_after_release, Some(1));
    }

    #[test]
    fn mutating_sql_helpers_commit_with_explicit_transaction() {
        let conn = open_sql_db();
        create_business_table(&conn);

        let bootstrapped: i64 = conn
            .query_row("SELECT bouncer_bootstrap()", [], |row| row.get(0))
            .unwrap();
        assert_eq!(bootstrapped, 1);

        conn.execute_batch("BEGIN").unwrap();
        conn.execute(
            "INSERT INTO business_events(note) VALUES (?1)",
            params!["claimed in tx"],
        )
        .unwrap();

        let claimed = conn
            .query_row(
                "SELECT bouncer_claim(?1, ?2, ?3, ?4)",
                params!["scheduler", "worker-a", 50_i64, 100_i64],
                |row| row.get::<_, Option<i64>>(0),
            )
            .expect("claim inside explicit transaction should succeed");
        assert_eq!(claimed, Some(1));

        conn.execute_batch("COMMIT").unwrap();

        assert_eq!(business_event_count(&conn), 1);
        assert_eq!(
            owner(&conn, "scheduler", 120).unwrap(),
            Some("worker-a".to_owned())
        );
        assert_eq!(token(&conn, "scheduler").unwrap(), Some(1));
    }

    #[test]
    fn mutating_sql_helpers_rollback_with_explicit_transaction() {
        let conn = open_sql_db();
        create_business_table(&conn);

        let bootstrapped: i64 = conn
            .query_row("SELECT bouncer_bootstrap()", [], |row| row.get(0))
            .unwrap();
        assert_eq!(bootstrapped, 1);

        conn.execute_batch("BEGIN").unwrap();
        conn.execute(
            "INSERT INTO business_events(note) VALUES (?1)",
            params!["rolled back"],
        )
        .unwrap();

        let claimed = conn
            .query_row(
                "SELECT bouncer_claim(?1, ?2, ?3, ?4)",
                params!["scheduler", "worker-a", 50_i64, 100_i64],
                |row| row.get::<_, Option<i64>>(0),
            )
            .expect("claim inside explicit transaction should succeed");
        assert_eq!(claimed, Some(1));

        conn.execute_batch("ROLLBACK").unwrap();

        assert_eq!(business_event_count(&conn), 0);
        assert_eq!(owner(&conn, "scheduler", 120).unwrap(), None);
        assert_eq!(token(&conn, "scheduler").unwrap(), None);
    }

    #[test]
    fn multiple_sql_mutators_commit_together_inside_explicit_transaction() {
        let conn = open_sql_db();

        let bootstrapped: i64 = conn
            .query_row("SELECT bouncer_bootstrap()", [], |row| row.get(0))
            .unwrap();
        assert_eq!(bootstrapped, 1);

        conn.execute_batch("BEGIN").unwrap();

        let scheduler = conn
            .query_row(
                "SELECT bouncer_claim(?1, ?2, ?3, ?4)",
                params!["scheduler", "worker-a", 50_i64, 100_i64],
                |row| row.get::<_, Option<i64>>(0),
            )
            .unwrap();
        let janitor = conn
            .query_row(
                "SELECT bouncer_claim(?1, ?2, ?3, ?4)",
                params!["janitor", "worker-b", 60_i64, 110_i64],
                |row| row.get::<_, Option<i64>>(0),
            )
            .unwrap();
        assert_eq!(scheduler, Some(1));
        assert_eq!(janitor, Some(1));

        conn.execute_batch("COMMIT").unwrap();

        assert_eq!(
            owner(&conn, "scheduler", 120).unwrap(),
            Some("worker-a".to_owned())
        );
        assert_eq!(
            owner(&conn, "janitor", 120).unwrap(),
            Some("worker-b".to_owned())
        );
    }

    #[test]
    fn multiple_sql_mutators_rollback_together_inside_explicit_transaction() {
        let conn = open_sql_db();

        let bootstrapped: i64 = conn
            .query_row("SELECT bouncer_bootstrap()", [], |row| row.get(0))
            .unwrap();
        assert_eq!(bootstrapped, 1);

        conn.execute_batch("BEGIN").unwrap();

        let scheduler = conn
            .query_row(
                "SELECT bouncer_claim(?1, ?2, ?3, ?4)",
                params!["scheduler", "worker-a", 50_i64, 100_i64],
                |row| row.get::<_, Option<i64>>(0),
            )
            .unwrap();
        let janitor = conn
            .query_row(
                "SELECT bouncer_claim(?1, ?2, ?3, ?4)",
                params!["janitor", "worker-b", 60_i64, 110_i64],
                |row| row.get::<_, Option<i64>>(0),
            )
            .unwrap();
        assert_eq!(scheduler, Some(1));
        assert_eq!(janitor, Some(1));

        conn.execute_batch("ROLLBACK").unwrap();

        assert_eq!(owner(&conn, "scheduler", 120).unwrap(), None);
        assert_eq!(owner(&conn, "janitor", 120).unwrap(), None);
        assert_eq!(token(&conn, "scheduler").unwrap(), None);
        assert_eq!(token(&conn, "janitor").unwrap(), None);
    }

    #[test]
    fn sql_read_helpers_work_inside_explicit_transaction() {
        let conn = open_sql_db();

        let bootstrapped: i64 = conn
            .query_row("SELECT bouncer_bootstrap()", [], |row| row.get(0))
            .unwrap();
        assert_eq!(bootstrapped, 1);

        conn.execute_batch("BEGIN").unwrap();

        let claimed = conn
            .query_row(
                "SELECT bouncer_claim(?1, ?2, ?3, ?4)",
                params!["scheduler", "worker-a", 50_i64, 100_i64],
                |row| row.get::<_, Option<i64>>(0),
            )
            .unwrap();
        assert_eq!(claimed, Some(1));

        let owner_in_tx: Option<String> = conn
            .query_row(
                "SELECT bouncer_owner(?1, ?2)",
                params!["scheduler", 120_i64],
                |row| row.get(0),
            )
            .unwrap();
        let token_in_tx: Option<i64> = conn
            .query_row("SELECT bouncer_token(?1)", params!["scheduler"], |row| {
                row.get(0)
            })
            .unwrap();

        assert_eq!(owner_in_tx.as_deref(), Some("worker-a"));
        assert_eq!(token_in_tx, Some(1));

        conn.execute_batch("ROLLBACK").unwrap();
    }

    #[test]
    fn sql_mutators_preserve_lease_semantics_inside_explicit_transaction() {
        let conn = open_sql_db();

        let bootstrapped: i64 = conn
            .query_row("SELECT bouncer_bootstrap()", [], |row| row.get(0))
            .unwrap();
        assert_eq!(bootstrapped, 1);

        conn.execute_batch("BEGIN").unwrap();

        let first_claim: Option<i64> = conn
            .query_row(
                "SELECT bouncer_claim(?1, ?2, ?3, ?4)",
                params!["scheduler", "worker-a", 50_i64, 100_i64],
                |row| row.get(0),
            )
            .unwrap();
        let busy_claim: Option<i64> = conn
            .query_row(
                "SELECT bouncer_claim(?1, ?2, ?3, ?4)",
                params!["scheduler", "worker-b", 30_i64, 120_i64],
                |row| row.get(0),
            )
            .unwrap();
        let takeover_claim: Option<i64> = conn
            .query_row(
                "SELECT bouncer_claim(?1, ?2, ?3, ?4)",
                params!["scheduler", "worker-b", 30_i64, 151_i64],
                |row| row.get(0),
            )
            .unwrap();
        let released: i64 = conn
            .query_row(
                "SELECT bouncer_release(?1, ?2, ?3)",
                params!["scheduler", "worker-b", 160_i64],
                |row| row.get(0),
            )
            .unwrap();
        let reclaimed: Option<i64> = conn
            .query_row(
                "SELECT bouncer_claim(?1, ?2, ?3, ?4)",
                params!["scheduler", "worker-c", 40_i64, 161_i64],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(first_claim, Some(1));
        assert_eq!(busy_claim, None);
        assert_eq!(takeover_claim, Some(2));
        assert_eq!(released, 1);
        assert_eq!(reclaimed, Some(3));

        conn.execute_batch("COMMIT").unwrap();

        assert_eq!(
            owner(&conn, "scheduler", 170).unwrap(),
            Some("worker-c".to_owned())
        );
        assert_eq!(token(&conn, "scheduler").unwrap(), Some(3));
    }

    #[test]
    fn sql_mutators_work_inside_savepoint_context() {
        let conn = open_sql_db();

        let bootstrapped: i64 = conn
            .query_row("SELECT bouncer_bootstrap()", [], |row| row.get(0))
            .unwrap();
        assert_eq!(bootstrapped, 1);

        conn.execute_batch("SAVEPOINT lease_ops").unwrap();

        let claimed = conn
            .query_row(
                "SELECT bouncer_claim(?1, ?2, ?3, ?4)",
                params!["scheduler", "worker-a", 50_i64, 100_i64],
                |row| row.get::<_, Option<i64>>(0),
            )
            .unwrap();
        assert_eq!(claimed, Some(1));

        conn.execute_batch("ROLLBACK TO lease_ops").unwrap();
        conn.execute_batch("RELEASE lease_ops").unwrap();

        assert_eq!(owner(&conn, "scheduler", 120).unwrap(), None);
    }

    fn release_after_claim(conn: &Connection) {
        claim(conn, "scheduler", "worker-a", 100, 50).unwrap();
        let released = release(conn, "scheduler", "worker-a", 120).unwrap();
        assert_eq!(
            released,
            ReleaseResult::Released {
                name: "scheduler".to_owned(),
                token: 1,
            }
        );
    }
}
