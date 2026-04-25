pub const BOUNCER_SCHEMA_VERSION: &str = "0";

/// Placeholder bootstrap for Bouncer's future SQLite contract.
///
/// The eventual crate should own:
/// - schema bootstrap
/// - lease / renew / release operations
/// - fencing token semantics
pub fn bootstrap_bouncer_schema() {}
