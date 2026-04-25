use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};

pub const BOUNCER_SCHEMA_VERSION: &str = "1";

pub type Result<T> = std::result::Result<T, Error>;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("database error: {0}")]
    Sqlite(#[from] rusqlite::Error),
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

/// Attempt to claim a named resource for `owner`.
pub fn claim(
    conn: &Connection,
    name: &str,
    owner: &str,
    now_ms: i64,
    ttl_ms: i64,
) -> Result<ClaimResult> {
    let lease_expires_at_ms = checked_expiry(now_ms, ttl_ms)?;
    let tx = begin_immediate(conn)?;

    let result = match load_resource(&tx, name)? {
        None => {
            tx.execute(
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

            ClaimResult::Acquired(LeaseInfo {
                name: name.to_owned(),
                owner: owner.to_owned(),
                token: 1,
                lease_expires_at_ms,
            })
        }
        Some(row) => match row.current_lease(now_ms)? {
            Some(current) => ClaimResult::Busy(current),
            None => {
                let next_token = row
                    .token
                    .checked_add(1)
                    .ok_or_else(|| Error::TokenOverflow(row.name.clone()))?;

                tx.execute(
                    "UPDATE bouncer_resources
                     SET owner = ?2,
                         token = ?3,
                         lease_expires_at_ms = ?4,
                         updated_at_ms = ?5
                     WHERE name = ?1",
                    params![name, owner, next_token, lease_expires_at_ms, now_ms],
                )?;

                ClaimResult::Acquired(LeaseInfo {
                    name: row.name,
                    owner: owner.to_owned(),
                    token: next_token,
                    lease_expires_at_ms,
                })
            }
        },
    };

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
    let lease_expires_at_ms = checked_expiry(now_ms, ttl_ms)?;
    let tx = begin_immediate(conn)?;

    let result = match load_resource(&tx, name)? {
        None => RenewResult::Rejected { current: None },
        Some(row) => match row.current_lease(now_ms)? {
            Some(current) if current.owner == owner => {
                tx.execute(
                    "UPDATE bouncer_resources
                     SET lease_expires_at_ms = ?2,
                         updated_at_ms = ?3
                     WHERE name = ?1",
                    params![name, lease_expires_at_ms, now_ms],
                )?;

                RenewResult::Renewed(LeaseInfo {
                    lease_expires_at_ms,
                    ..current
                })
            }
            Some(current) => RenewResult::Rejected {
                current: Some(current),
            },
            None => RenewResult::Rejected { current: None },
        },
    };

    tx.commit()?;
    Ok(result)
}

/// Release a currently-live lease owned by `owner`.
pub fn release(conn: &Connection, name: &str, owner: &str, now_ms: i64) -> Result<ReleaseResult> {
    let tx = begin_immediate(conn)?;

    let result = match load_resource(&tx, name)? {
        None => ReleaseResult::Rejected { current: None },
        Some(row) => match row.current_lease(now_ms)? {
            Some(current) if current.owner == owner => {
                tx.execute(
                    "UPDATE bouncer_resources
                     SET owner = NULL,
                         lease_expires_at_ms = NULL,
                         updated_at_ms = ?2
                     WHERE name = ?1",
                    params![name, now_ms],
                )?;

                ReleaseResult::Released {
                    name: current.name,
                    token: current.token,
                }
            }
            Some(current) => ReleaseResult::Rejected {
                current: Some(current),
            },
            None => ReleaseResult::Rejected { current: None },
        },
    };

    tx.commit()?;
    Ok(result)
}

fn begin_immediate(conn: &Connection) -> rusqlite::Result<Transaction<'_>> {
    Transaction::new_unchecked(conn, TransactionBehavior::Immediate)
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
