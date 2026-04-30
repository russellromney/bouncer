//! Phase 013 schema and data-integrity hardening suite.
//!
//! Every row uses a fresh file-backed database so the test surface matches
//! real persisted-state failure modes.
//!
//! Scope is pinned by `spec-diff.md` and the `[D1]`–`[D21]` decisions:
//!
//! - bootstrap-folded schema validation, strict not affinity-tolerant
//! - deliberate `Error::SchemaMismatch { reason }` (variant-stable, reason
//!   diagnostic only)
//! - invalid persisted rows surfaced through public API reads/mutators
//!   using deliberately broken schema fixtures (no `writable_schema`)
//! - overflow and TTL edges, asserted non-mutating on failure
//! - opaque text round-trip for empty/whitespace/newlines/Unicode/punctuation
//!   plus a 4 KiB UTF-8 string

use std::path::{Path, PathBuf};

use bouncer_core::{
    bootstrap_bouncer_schema, claim, inspect, owner as core_owner, release, renew, token,
    ClaimResult, Error, LeaseInfo, ReleaseResult, RenewResult,
};
use rusqlite::{params, Connection};
use tempfile::TempDir;

const NAME: &str = "scheduler";
const OWNER_A: &str = "worker-a";
const OWNER_B: &str = "worker-b";
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

fn open_conn(path: &Path) -> Connection {
    Connection::open(path).expect("open sqlite connection")
}

fn open_bootstrapped(path: &Path) -> Connection {
    let conn = open_conn(path);
    bootstrap_bouncer_schema(&conn).expect("bootstrap schema");
    conn
}

fn assert_schema_mismatch(result: bouncer_core::Result<()>) -> String {
    match result {
        Err(Error::SchemaMismatch { reason }) => reason,
        Err(other) => panic!("expected SchemaMismatch, got {other:?}"),
        Ok(()) => panic!("expected SchemaMismatch, got Ok(())"),
    }
}

// -------------------------- schema validation: positive --------------------

#[test]
fn bootstrap_creates_fresh_schema_when_table_absent() {
    let db = DbFile::fresh();
    let conn = open_conn(&db.path);

    bootstrap_bouncer_schema(&conn).expect("bootstrap fresh DB");

    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master \
             WHERE type='table' AND name='bouncer_resources'",
            [],
            |row| row.get(0),
        )
        .expect("count");
    assert_eq!(count, 1);

    // Lease ops work afterward.
    let result = claim(&conn, NAME, OWNER_A, NOW_MS, TTL_MS).expect("claim");
    assert!(matches!(result, ClaimResult::Acquired(_)));
}

#[test]
fn bootstrap_is_idempotent_on_valid_schema() {
    let db = DbFile::fresh();
    let conn = open_bootstrapped(&db.path);

    bootstrap_bouncer_schema(&conn).expect("second bootstrap");
    bootstrap_bouncer_schema(&conn).expect("third bootstrap");
}

#[test]
fn bootstrap_preserves_existing_live_lease() {
    let db = DbFile::fresh();
    let conn = open_bootstrapped(&db.path);

    let claimed = claim(&conn, NAME, OWNER_A, NOW_MS, TTL_MS).expect("claim");
    let claimed_lease = match claimed {
        ClaimResult::Acquired(lease) => lease,
        other => panic!("expected acquired lease, got {other:?}"),
    };

    bootstrap_bouncer_schema(&conn).expect("re-bootstrap on valid schema");

    let still_live = inspect(&conn, NAME, NOW_MS + 1)
        .expect("inspect")
        .expect("live lease");
    assert_eq!(still_live, claimed_lease);
}

#[test]
fn bootstrap_inside_caller_owned_transaction_is_valid() {
    let db = DbFile::fresh();
    let conn = open_bootstrapped(&db.path);

    conn.execute_batch("BEGIN").expect("begin caller tx");
    bootstrap_bouncer_schema(&conn).expect("bootstrap inside tx is read-only");
    // Validation should not write, so caller transaction can still ROLLBACK
    // cleanly without losing anything.
    conn.execute_batch("ROLLBACK").expect("rollback caller tx");

    // Schema still intact afterward.
    bootstrap_bouncer_schema(&conn).expect("post-rollback bootstrap still valid");
}

// -------------------------- schema validation: negative --------------------

fn create_table_raw(conn: &Connection, ddl: &str) {
    conn.execute_batch(ddl).expect("install custom table ddl");
}

#[test]
fn bootstrap_rejects_table_with_extra_column() {
    let db = DbFile::fresh();
    let conn = open_conn(&db.path);
    create_table_raw(
        &conn,
        "CREATE TABLE bouncer_resources (
           name TEXT PRIMARY KEY,
           owner TEXT,
           token INTEGER NOT NULL CHECK (token >= 1),
           lease_expires_at_ms INTEGER,
           created_at_ms INTEGER NOT NULL,
           updated_at_ms INTEGER NOT NULL,
           notes TEXT,
           CHECK (
             (owner IS NULL AND lease_expires_at_ms IS NULL)
             OR (owner IS NOT NULL AND lease_expires_at_ms IS NOT NULL)
           )
         );",
    );

    let reason = assert_schema_mismatch(bootstrap_bouncer_schema(&conn));
    assert!(
        reason.contains("expected 6 columns, found 7"),
        "got {reason}"
    );
}

#[test]
fn bootstrap_rejects_table_missing_required_column() {
    let db = DbFile::fresh();
    let conn = open_conn(&db.path);
    create_table_raw(
        &conn,
        "CREATE TABLE bouncer_resources (
           name TEXT PRIMARY KEY,
           owner TEXT,
           token INTEGER NOT NULL CHECK (token >= 1),
           lease_expires_at_ms INTEGER,
           created_at_ms INTEGER NOT NULL
           -- missing updated_at_ms
         );",
    );

    let reason = assert_schema_mismatch(bootstrap_bouncer_schema(&conn));
    assert!(
        reason.contains("expected 6 columns, found 5"),
        "got {reason}"
    );
}

#[test]
fn bootstrap_rejects_table_with_swapped_column_order() {
    let db = DbFile::fresh();
    let conn = open_conn(&db.path);
    // owner and token swapped at positions 1 and 2.
    create_table_raw(
        &conn,
        "CREATE TABLE bouncer_resources (
           name TEXT PRIMARY KEY,
           token INTEGER NOT NULL CHECK (token >= 1),
           owner TEXT,
           lease_expires_at_ms INTEGER,
           created_at_ms INTEGER NOT NULL,
           updated_at_ms INTEGER NOT NULL,
           CHECK (
             (owner IS NULL AND lease_expires_at_ms IS NULL)
             OR (owner IS NOT NULL AND lease_expires_at_ms IS NOT NULL)
           )
         );",
    );

    let reason = assert_schema_mismatch(bootstrap_bouncer_schema(&conn));
    assert!(reason.contains("expected column `owner` at position 1"), "got {reason}");
}

#[test]
fn bootstrap_rejects_affinity_compatible_but_text_different_type() {
    let db = DbFile::fresh();
    let conn = open_conn(&db.path);
    // BIGINT has the same INTEGER affinity as INTEGER but differs in declared
    // text. Phase 013 picks strict text matching, so this is drift.
    create_table_raw(
        &conn,
        "CREATE TABLE bouncer_resources (
           name TEXT PRIMARY KEY,
           owner TEXT,
           token BIGINT NOT NULL CHECK (token >= 1),
           lease_expires_at_ms INTEGER,
           created_at_ms INTEGER NOT NULL,
           updated_at_ms INTEGER NOT NULL,
           CHECK (
             (owner IS NULL AND lease_expires_at_ms IS NULL)
             OR (owner IS NOT NULL AND lease_expires_at_ms IS NOT NULL)
           )
         );",
    );

    let reason = assert_schema_mismatch(bootstrap_bouncer_schema(&conn));
    assert!(
        reason.contains("column `token` declared type mismatch"),
        "got {reason}"
    );
    assert!(reason.contains("BIGINT"), "got {reason}");
}

#[test]
fn bootstrap_rejects_table_with_wrong_nullability() {
    let db = DbFile::fresh();
    let conn = open_conn(&db.path);
    // updated_at_ms missing NOT NULL.
    create_table_raw(
        &conn,
        "CREATE TABLE bouncer_resources (
           name TEXT PRIMARY KEY,
           owner TEXT,
           token INTEGER NOT NULL CHECK (token >= 1),
           lease_expires_at_ms INTEGER,
           created_at_ms INTEGER NOT NULL,
           updated_at_ms INTEGER,
           CHECK (
             (owner IS NULL AND lease_expires_at_ms IS NULL)
             OR (owner IS NOT NULL AND lease_expires_at_ms IS NOT NULL)
           )
         );",
    );

    let reason = assert_schema_mismatch(bootstrap_bouncer_schema(&conn));
    assert!(
        reason.contains("column `updated_at_ms` NOT NULL mismatch"),
        "got {reason}"
    );
}

#[test]
fn bootstrap_rejects_table_with_wrong_primary_key() {
    let db = DbFile::fresh();
    let conn = open_conn(&db.path);
    // name is no longer PRIMARY KEY.
    create_table_raw(
        &conn,
        "CREATE TABLE bouncer_resources (
           name TEXT,
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
    );

    let reason = assert_schema_mismatch(bootstrap_bouncer_schema(&conn));
    assert!(
        reason.contains("column `name` primary-key position mismatch"),
        "got {reason}"
    );
}

#[test]
fn bootstrap_rejects_table_without_token_check() {
    let db = DbFile::fresh();
    let conn = open_conn(&db.path);
    // Drop the token CHECK while keeping every column shape correct.
    create_table_raw(
        &conn,
        "CREATE TABLE bouncer_resources (
           name TEXT PRIMARY KEY,
           owner TEXT,
           token INTEGER NOT NULL,
           lease_expires_at_ms INTEGER,
           created_at_ms INTEGER NOT NULL,
           updated_at_ms INTEGER NOT NULL,
           CHECK (
             (owner IS NULL AND lease_expires_at_ms IS NULL)
             OR (owner IS NOT NULL AND lease_expires_at_ms IS NOT NULL)
           )
         );",
    );

    let reason = assert_schema_mismatch(bootstrap_bouncer_schema(&conn));
    assert!(
        reason.contains("CHECK (token >= 1)"),
        "got {reason}"
    );
}

#[test]
fn bootstrap_rejects_table_without_pair_check() {
    let db = DbFile::fresh();
    let conn = open_conn(&db.path);
    // Drop the table-level pair CHECK while keeping the token CHECK.
    create_table_raw(
        &conn,
        "CREATE TABLE bouncer_resources (
           name TEXT PRIMARY KEY,
           owner TEXT,
           token INTEGER NOT NULL CHECK (token >= 1),
           lease_expires_at_ms INTEGER,
           created_at_ms INTEGER NOT NULL,
           updated_at_ms INTEGER NOT NULL
         );",
    );

    let reason = assert_schema_mismatch(bootstrap_bouncer_schema(&conn));
    assert!(reason.contains("owner IS NULL"), "got {reason}");
    assert!(reason.contains("lease_expires_at_ms"), "got {reason}");
}

// -------------------------- invalid persisted rows -------------------------

/// Create a `bouncer_resources` table without the row-level CHECK
/// constraints so tests can persist rows that Bouncer must reject. This
/// fixture deliberately produces a schema Bouncer's bootstrap would reject;
/// it bypasses bootstrap entirely.
fn create_unchecked_resources_table(conn: &Connection) {
    conn.execute_batch(
        "CREATE TABLE bouncer_resources (
           name TEXT PRIMARY KEY,
           owner TEXT,
           token INTEGER NOT NULL,
           lease_expires_at_ms INTEGER,
           created_at_ms INTEGER NOT NULL,
           updated_at_ms INTEGER NOT NULL
         );",
    )
    .expect("install unchecked schema");
}

fn insert_raw_row(
    conn: &Connection,
    name: &str,
    owner: Option<&str>,
    token: i64,
    expires: Option<i64>,
) {
    conn.execute(
        "INSERT INTO bouncer_resources \
         (name, owner, token, lease_expires_at_ms, created_at_ms, updated_at_ms) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![name, owner, token, expires, NOW_MS, NOW_MS],
    )
    .expect("insert raw row");
}

fn assert_invalid_lease_row(result: bouncer_core::Result<impl std::fmt::Debug>) {
    match result {
        Err(Error::InvalidLeaseRow(name)) => assert_eq!(name, NAME),
        other => panic!("expected InvalidLeaseRow, got {other:?}"),
    }
}

#[test]
fn inspect_rejects_row_with_owner_but_no_expiry() {
    let db = DbFile::fresh();
    let conn = open_conn(&db.path);
    create_unchecked_resources_table(&conn);
    insert_raw_row(&conn, NAME, Some(OWNER_A), 1, None);

    assert_invalid_lease_row(inspect(&conn, NAME, NOW_MS));
}

#[test]
fn inspect_rejects_row_with_expiry_but_no_owner() {
    let db = DbFile::fresh();
    let conn = open_conn(&db.path);
    create_unchecked_resources_table(&conn);
    insert_raw_row(&conn, NAME, None, 1, Some(NOW_MS + TTL_MS));

    assert_invalid_lease_row(inspect(&conn, NAME, NOW_MS));
}

#[test]
fn inspect_rejects_row_with_zero_token() {
    let db = DbFile::fresh();
    let conn = open_conn(&db.path);
    create_unchecked_resources_table(&conn);
    insert_raw_row(&conn, NAME, None, 0, None);

    assert_invalid_lease_row(inspect(&conn, NAME, NOW_MS));
}

#[test]
fn claim_rejects_row_with_invalid_state() {
    let db = DbFile::fresh();
    let conn = open_conn(&db.path);
    create_unchecked_resources_table(&conn);
    insert_raw_row(&conn, NAME, Some(OWNER_A), 1, None);

    assert_invalid_lease_row(claim(&conn, NAME, OWNER_B, NOW_MS, TTL_MS));
}

#[test]
fn renew_rejects_row_with_invalid_state() {
    let db = DbFile::fresh();
    let conn = open_conn(&db.path);
    create_unchecked_resources_table(&conn);
    insert_raw_row(&conn, NAME, Some(OWNER_A), 1, None);

    assert_invalid_lease_row(renew(&conn, NAME, OWNER_A, NOW_MS, TTL_MS));
}

#[test]
fn release_rejects_row_with_invalid_state() {
    let db = DbFile::fresh();
    let conn = open_conn(&db.path);
    create_unchecked_resources_table(&conn);
    insert_raw_row(&conn, NAME, Some(OWNER_A), 1, None);

    assert_invalid_lease_row(release(&conn, NAME, OWNER_A, NOW_MS));
}

// -------------------------- token near-overflow ----------------------------

fn read_raw_token(conn: &Connection, name: &str) -> i64 {
    conn.query_row(
        "SELECT token FROM bouncer_resources WHERE name = ?1",
        params![name],
        |row| row.get(0),
    )
    .expect("read token")
}

#[test]
fn claim_takeover_fails_at_token_max_without_mutating_row() {
    let db = DbFile::fresh();
    let conn = open_bootstrapped(&db.path);

    // Seed a row with token = i64::MAX in the released state.
    conn.execute(
        "INSERT INTO bouncer_resources \
         (name, owner, token, lease_expires_at_ms, created_at_ms, updated_at_ms) \
         VALUES (?1, NULL, ?2, NULL, ?3, ?3)",
        params![NAME, i64::MAX, NOW_MS],
    )
    .expect("seed max-token released row");

    let err = claim(&conn, NAME, OWNER_A, NOW_MS + 1, TTL_MS).unwrap_err();
    assert!(matches!(err, Error::TokenOverflow(ref n) if n == NAME));

    // Row should be untouched.
    assert_eq!(read_raw_token(&conn, NAME), i64::MAX);
    assert_eq!(token(&conn, NAME).expect("token"), Some(i64::MAX));
    assert_eq!(inspect(&conn, NAME, NOW_MS + 1).expect("inspect"), None);
}

#[test]
fn claim_takeover_at_token_max_minus_one_succeeds_to_max() {
    let db = DbFile::fresh();
    let conn = open_bootstrapped(&db.path);

    conn.execute(
        "INSERT INTO bouncer_resources \
         (name, owner, token, lease_expires_at_ms, created_at_ms, updated_at_ms) \
         VALUES (?1, NULL, ?2, NULL, ?3, ?3)",
        params![NAME, i64::MAX - 1, NOW_MS],
    )
    .expect("seed near-max released row");

    let result = claim(&conn, NAME, OWNER_A, NOW_MS + 1, TTL_MS).expect("claim");
    match result {
        ClaimResult::Acquired(lease) => assert_eq!(lease.token, i64::MAX),
        other => panic!("expected Acquired at i64::MAX, got {other:?}"),
    }

    // A second takeover at MAX should now overflow without mutation.
    let err = claim(
        &conn,
        NAME,
        OWNER_B,
        NOW_MS + i64::from(TTL_MS as i32) + 100,
        TTL_MS,
    )
    .unwrap_err();
    assert!(matches!(err, Error::TokenOverflow(ref n) if n == NAME));
    assert_eq!(read_raw_token(&conn, NAME), i64::MAX);
}

// -------------------------- TTL edges --------------------------------------

#[test]
fn claim_rejects_zero_ttl() {
    let db = DbFile::fresh();
    let conn = open_bootstrapped(&db.path);
    let err = claim(&conn, NAME, OWNER_A, NOW_MS, 0).unwrap_err();
    assert!(matches!(err, Error::InvalidTtlMs(0)));
}

#[test]
fn claim_rejects_negative_ttl() {
    let db = DbFile::fresh();
    let conn = open_bootstrapped(&db.path);
    let err = claim(&conn, NAME, OWNER_A, NOW_MS, -42).unwrap_err();
    assert!(matches!(err, Error::InvalidTtlMs(-42)));
}

#[test]
fn renew_rejects_zero_ttl() {
    let db = DbFile::fresh();
    let conn = open_bootstrapped(&db.path);
    claim(&conn, NAME, OWNER_A, NOW_MS, TTL_MS).expect("seed claim");
    let err = renew(&conn, NAME, OWNER_A, NOW_MS + 1, 0).unwrap_err();
    assert!(matches!(err, Error::InvalidTtlMs(0)));
}

#[test]
fn renew_rejects_negative_ttl() {
    let db = DbFile::fresh();
    let conn = open_bootstrapped(&db.path);
    claim(&conn, NAME, OWNER_A, NOW_MS, TTL_MS).expect("seed claim");
    let err = renew(&conn, NAME, OWNER_A, NOW_MS + 1, -1).unwrap_err();
    assert!(matches!(err, Error::InvalidTtlMs(-1)));
}

#[test]
fn claim_rejects_ttl_overflow_at_i64_boundary() {
    let db = DbFile::fresh();
    let conn = open_bootstrapped(&db.path);
    let err = claim(&conn, NAME, OWNER_A, i64::MAX, 1).unwrap_err();
    assert!(matches!(err, Error::TtlOverflow { now_ms, ttl_ms }
        if now_ms == i64::MAX && ttl_ms == 1));

    // Row was not created.
    assert_eq!(token(&conn, NAME).expect("token"), None);
}

#[test]
fn renew_rejects_ttl_overflow_at_i64_boundary() {
    let db = DbFile::fresh();
    let conn = open_bootstrapped(&db.path);
    claim(&conn, NAME, OWNER_A, NOW_MS, TTL_MS).expect("seed claim");

    let err = renew(&conn, NAME, OWNER_A, i64::MAX, 1).unwrap_err();
    assert!(matches!(err, Error::TtlOverflow { now_ms, ttl_ms }
        if now_ms == i64::MAX && ttl_ms == 1));

    // Existing live lease still observable at the original time.
    let lease = inspect(&conn, NAME, NOW_MS + 1)
        .expect("inspect")
        .expect("live lease");
    assert_eq!(lease.lease_expires_at_ms, NOW_MS + TTL_MS);
}

// -------------------------- unusual text round-trip ------------------------

const LONG_STRING_BYTES: usize = 4 * 1024;

fn long_utf8_string() -> String {
    // 4 KiB of mixed ASCII + multi-byte UTF-8. Use a small repeating pattern
    // so the exact byte length is deterministic.
    let unit = "abc \u{00e9}\u{6587}\u{1F600} "; // 12 bytes per unit
    let mut out = String::with_capacity(LONG_STRING_BYTES);
    while out.len() + unit.len() <= LONG_STRING_BYTES {
        out.push_str(unit);
    }
    while out.len() < LONG_STRING_BYTES {
        out.push(' ');
    }
    assert_eq!(out.len(), LONG_STRING_BYTES);
    out
}

fn assert_round_trip(name: &str, owner: &str) {
    let db = DbFile::fresh();
    let conn = open_bootstrapped(&db.path);

    let claimed = claim(&conn, name, owner, NOW_MS, TTL_MS).expect("claim unusual");
    match &claimed {
        ClaimResult::Acquired(lease) => {
            assert_eq!(lease.name, name);
            assert_eq!(lease.owner, owner);
            assert_eq!(lease.token, 1);
            assert_eq!(lease.lease_expires_at_ms, NOW_MS + TTL_MS);
        }
        other => panic!("expected Acquired, got {other:?}"),
    }

    let inspected = inspect(&conn, name, NOW_MS + 1)
        .expect("inspect")
        .expect("live");
    assert_eq!(inspected.owner, owner);

    let core_owner_str = core_owner(&conn, name, NOW_MS + 1)
        .expect("owner")
        .expect("live owner");
    assert_eq!(core_owner_str, owner);

    let tok = token(&conn, name).expect("token").expect("token row");
    assert_eq!(tok, 1);

    let renewed = renew(&conn, name, owner, NOW_MS + 1, TTL_MS).expect("renew");
    assert!(matches!(renewed, RenewResult::Renewed(_)));

    let released = release(&conn, name, owner, NOW_MS + 2).expect("release");
    assert_eq!(
        released,
        ReleaseResult::Released {
            name: name.to_owned(),
            token: 1,
        }
    );

    // Token preserved across release.
    assert_eq!(token(&conn, name).expect("token"), Some(1));

    // Reclaim by a fresh owner advances the token monotonically.
    let reclaimed = claim(&conn, name, OWNER_A, NOW_MS + 3, TTL_MS).expect("reclaim");
    match reclaimed {
        ClaimResult::Acquired(LeaseInfo { token, .. }) => assert_eq!(token, 2),
        other => panic!("expected Acquired token=2, got {other:?}"),
    }
}

#[test]
fn round_trip_empty_strings() {
    // Empty owner is allowed; empty name is also allowed since it's a TEXT
    // primary key value, not a NULL.
    assert_round_trip("", "");
}

#[test]
fn round_trip_whitespace_only_strings() {
    assert_round_trip("  leading and trailing  ", "\towner with tab\t");
}

#[test]
fn round_trip_strings_with_newlines_and_tabs() {
    assert_round_trip("name\nwith\nnewlines", "owner\twith\ttabs\nand newline");
}

#[test]
fn round_trip_unicode_and_combining_marks() {
    // emoji + combining acute + CJK.
    assert_round_trip("scheduler-\u{1F680}-cafe\u{0301}", "ow\u{6E2C}\u{8A66}-\u{1F600}");
}

#[test]
fn round_trip_punctuation_and_sql_shaped_strings() {
    assert_round_trip(
        "a/b/c, scheduler #1",
        "'; DROP TABLE bouncer_resources; --",
    );
}

#[test]
fn round_trip_long_4kib_utf8_string() {
    let long = long_utf8_string();
    assert_round_trip(&long, &long);
}
