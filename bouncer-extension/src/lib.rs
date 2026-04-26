//! Bouncer SQLite loadable extension.
//!
//! Thin wrapper around `bouncer-honker`. Registers the first
//! `bouncer_*` SQL scalar functions so any SQLite client can answer:
//! "who owns this named resource right now?"

use rusqlite::ffi;
use rusqlite::Connection;
use std::os::raw::{c_char, c_int};

/// SQLite entry point. Name must match `sqlite3_<extname>_init`.
///
/// `libbouncer_ext.dylib` -> `bouncer_ext` -> `bouncerext`
/// -> `sqlite3_bouncerext_init`.
///
/// # Safety
/// Called by SQLite. All pointers are SQLite-owned.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sqlite3_bouncerext_init(
    db: *mut ffi::sqlite3,
    pz_err_msg: *mut *mut c_char,
    p_api: *mut ffi::sqlite3_api_routines,
) -> c_int {
    unsafe {
        Connection::extension_init2(db, pz_err_msg, p_api, |conn| {
            bouncer_honker::attach_bouncer_functions(&conn)?;
            Ok(true)
        })
    }
}
