use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use bouncer_core as core;
use rusqlite::{
    Connection, Savepoint as SqlSavepoint, Transaction as SqlTransaction, TransactionBehavior,
};

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

#[derive(Debug)]
pub struct Savepoint<'db> {
    sp: SqlSavepoint<'db>,
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

    pub fn savepoint(&mut self) -> Result<Savepoint<'_>> {
        let sp = self.tx.savepoint()?;
        Ok(Savepoint { sp })
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

impl<'db> Savepoint<'db> {
    pub fn conn(&self) -> &Connection {
        &self.sp
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
        self.sp.commit()?;
        Ok(())
    }

    pub fn rollback(mut self) -> Result<()> {
        self.sp.rollback()?;
        self.sp.commit()?;
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
mod tests;
