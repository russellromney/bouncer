# Changelog

## Unreleased

### Added

- Added the initial repo scaffold for `bouncer`.
- Added the first real `bouncer-honker` Rust core crate implementation.
- Added the first SQLite schema bootstrap for `bouncer_resources`.
- Added Rust `claim`, `renew`, `release`, and time-aware `inspect` helpers.
- Added Phase 001 tests for claim, expiry, renew, release, and monotonic fencing behavior.
- Added the first pass of `README.md`, `ROADMAP.md`, and `SYSTEM.md` to capture product intent before implementation.
- Added `.intent/phases/001-core-lease-contract/` with spec, plan, review/decision, and commit-trace artifacts.

### Changed

- Clarified that Bouncer is a single-machine lease / fencing primitive for SQLite apps, not a distributed coordination system.
- Clarified that Phase 001 stops at the Rust core contract and tests; bindings remain future work.
- Clarified the repo's phase workflow around `spec-diff.md`, `plan.md`, and `reviews_and_decisions.md`.
