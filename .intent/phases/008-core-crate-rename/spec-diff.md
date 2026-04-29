What changes:

- The Rust core crate and directory are renamed from
  `bouncer-honker` to `bouncer-core`.
- Current docs, dependency declarations, imports, and future-phase
  plans use `bouncer-core`.
- The Python binding phase is renumbered to Phase 009 so the rename
  lands before another binding depends on the old crate name.

What does not change:

- Lease semantics do not change.
- SQLite schema and SQL function names do not change.
- The Rust wrapper public API does not change.
- The SQLite extension public surface does not change.
- Historical intent artifacts are not rewritten to hide the old name.

How we will verify it:

- `cargo test -p bouncer-core -p bouncer`
- `cargo build -p bouncer-extension`
- `cargo clippy --workspace --all-targets -- -D warnings`
- current docs and active plans no longer describe the core as
  `bouncer-honker`
