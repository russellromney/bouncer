# Reviews And Decisions

This file is append-only.

- Review rounds are written in Session B.
- Decision rounds are written in Session A in response to findings.
- Do not rewrite earlier review text to make it look resolved.

## Review Round 001

Target:
- plan review

Session:
- B

Model family:
- Claude Opus 4.7

Artifacts reviewed:
- `spec-diff.md`
- `plan.md`
- `SYSTEM.md` (Phase 001 baseline, for cross-check)
- `bouncer-honker/src/lib.rs` (only to confirm the surface the wrapper
  is wrapping)

Verification reviewed:
- planned test matrix only

### Positive conformance review

- [P1] The plan correctly stops at one in-process Rust crate. The spec
  diff says "first thin Rust binding crate," and the plan does not slip
  into Python, Node, or a SQLite loadable extension. The non-goals
  section restates the spec-diff exclusions cleanly.
- [P2] The plan preserves Phase 001's explicit-time core as the source
  of truth and treats system time as wrapper-only sugar. This matches
  the spec diff phrase "leaving the lower-level core crate available
  for deterministic tests and advanced use," and avoids the trap of
  baking wall-clock dependence into the contract.
- [P3] The plan's interop test description (binding writes visible to
  core, core writes visible to binding) is the right shape to satisfy
  the spec-diff verification line "binding and `bouncer-honker`
  interoperate against the same database file." It also leans on the
  Phase 001 file-backed multi-connection work rather than redoing it.
- [P4] The Phase decisions explicitly forbid bootstrapping by hidden
  global side effect. That is precisely the failure mode that would let
  a "thin" binding silently re-invent semantics, and naming it up front
  is the right defensive move.
- [P5] The plan keeps the wrapper public surface to four lease verbs
  plus open/from_connection. That maps 1:1 to the spec diff's "small
  ergonomic API for opening a SQLite database, bootstrapping Bouncer's
  schema, and calling `inspect`, `claim`, `renew`, and `release`."

### Negative conformance review

- [N1] Bootstrap behavior is underspecified. The spec diff lists
  "bootstrapping Bouncer's schema" as part of the binding's job, and
  the plan offers two options ("open path or an explicit helper") but
  does not commit. A reader of the plan cannot tell whether
  `Bouncer::open(path)` returns a fully-bootstrapped database or
  whether the caller must invoke a separate `bootstrap()`.
- [N2] Connection ownership is underspecified. The plan suggests both
  `Bouncer::open(path) -> Result<Self>` and
  `Bouncer::from_connection(conn) -> Result<Self>` "or a close
  variant," and floats a `Bouncer` / `BouncerRef<'a>` borrowed/owned
  split as optional.
- [N3] The system-time path is not pinned. The plan says tests should
  pin the wrapper "without sleeping or flaky timing where possible,"
  but never says how.
- [N4] The result-type mapping decision is deferred. The plan accepts
  "may wrap or re-export the core result shapes."
- [N5] Connection pragmas are not mentioned. If `Bouncer::open(path)`
  opens a Connection without setting `journal_mode`, `busy_timeout`,
  etc., the wrapper may quietly lose the contention guarantees the core
  was tested under.
- [N6] The test matrix is missing a few items the spec diff arguably
  requires: bootstrap idempotence through the wrapper, TTL-rejection
  parity with the core, and fencing-token monotonicity across a
  wrapper-claim and a raw-core-claim on the same file.

### Adversarial review

- [A1] This becomes dumb if "ergonomic API" turns out to mean four
  one-liners that call `SystemTime::now()` and forward to
  `bouncer-honker`, plus an `open` helper.
- [A2] This becomes self-deceiving if `Bouncer::open(path)` silently
  bootstraps the schema with no opt-out.
- [A3] This becomes misleading if the interop test runs the wrapper and
  the core inside the same process from the same in-memory database, or
  against a single Connection.
- [A4] The phase will not actually settle the binding shape if the
  plan stays this open.
- [A5] If Phase 002 is the prelude to non-Rust bindings, every shape
  decision made here will become a constraint on TS, Python, etc.

### Review verdict

- Accepted with follow-up decisions required before coding.

## Decision Round 001

Responding to:
- Review Round 001

Session:
- A

### Inputs

- [P1]
- [P2]
- [P3]
- [P4]
- [P5]
- [N1]
- [N2]
- [N3]
- [N4]
- [N5]
- [N6]
- [A1]
- [A2]
- [A3]
- [A4]
- [A5]

### Decisions

- [D1] Accept [N1] and [A2]
  Action: make bootstrap explicit and idempotent through `bootstrap()`;
  `open(path)` should not silently mutate schema state.
  Targets: `spec-diff.md`, `plan.md`

- [D2] Accept [N2] and [A4]
  Action: commit to an owned `Bouncer` wrapper plus a borrowed
  `BouncerRef<'a>` wrapper instead of leaving connection ownership open.
  Targets: `spec-diff.md`, `plan.md`

- [D3] Accept [N3]
  Action: expose explicit `*_at` wrapper variants and use those for
  deterministic tests instead of adding an injectable clock object.
  Targets: `spec-diff.md`, `plan.md`

- [D4] Accept [N4]
  Action: re-export core lease/result shapes rather than inventing a
  second semantic taxonomy in the wrapper.
  Targets: `spec-diff.md`, `plan.md`

- [D5] Accept [N5]
  Action: keep SQLite pragma policy out of Phase 002 and make the plan
  explicit that `open(path)` should not silently tune `journal_mode`,
  `busy_timeout`, or related connection settings.
  Targets: `plan.md`

- [D6] Accept [N6] and [A3]
  Action: expand the test matrix to include wrapper bootstrap
  idempotence, TTL-rejection parity, separate-connection wrapper/core
  interop, and fencing-token monotonicity across wrapper/core calls to
  the same file.
  Targets: `spec-diff.md`, `plan.md`

- [D7] Accept [A1]
  Action: keep the wrapper intentionally small and say so plainly rather
  than pretending this phase is larger than it is.
  Targets: implementation scope, docs tone

- [D8] Accept [A5]
  Action: treat the Rust wrapper shape as a likely template for future
  language bindings and therefore make the public shape explicit before
  coding.
  Targets: `spec-diff.md`, `plan.md`

### Verification

- `spec-diff.md` now makes bootstrap explicit, wrapper/core interop
  cross-connection, and result-shape reuse part of the phase contract.
- `plan.md` now commits to owned plus borrowed wrappers, explicit
  `*_at` variants, no hidden bootstrap, no hidden pragma policy, and the
  stronger interop test matrix.

### Decision verdict

- Ready for implementation review handoff or coding.

## Review Round 002

Target:
- decision review (Decision Round 001 plus the resulting
  `spec-diff.md` and `plan.md` revisions)

Session:
- B

Model family:
- Claude Opus 4.7

Artifacts reviewed:
- Decision Round 001 in this file
- `spec-diff.md` (revised)
- `plan.md` (revised)

Verification reviewed:
- planned test matrix only

### Positive conformance review

- [P6] [D1] and [D4] land cleanly in both `spec-diff.md` and
  `plan.md`. Bootstrap is now an explicit phrase in the spec
  ("explicitly bootstrapping Bouncer's schema") and the plan exposes
  `bootstrap()` as its own method on both wrapper types. Re-exporting
  core result shapes is now phrased as a Phase decision rather than
  an implementation taste call.
- [P7] [D5] is the cleanest of the eight: it shows up in the Phase
  decisions, in build order step 5, and in the "Areas that should
  not be touched" list as "hidden connection-policy changes." The
  three-place restatement makes accidental drift unlikely.
- [P8] [D6] turns the test matrix into something that can actually
  falsify the contract [A3] worried about. The plan now lists
  separate-connection interop, fencing-token monotonicity across
  wrapper/core, bootstrap idempotence, and TTL parity as named
  tests, not as "comprehensive coverage."
- [P9] [D2] commits to owned + borrowed wrappers and writes both
  types into the public shape, instead of leaving the
  borrowed/owned split as "if implementation argues for it." That
  closes the Phase 001-style ambiguity I flagged in [N2].
- [P10] [D3] picks the simpler of the two time-injection options
  (`*_at` variants over a clock object), which keeps the wrapper
  visually close to the core and avoids a new abstraction.

### Negative conformance review

- [N7] The expanded test matrix from [D6] does not include the
  *negative* assertion that [D1] introduced: that `open(path)` does
  **not** bootstrap, and that wrapper methods called before
  `bootstrap()` produce a clean error rather than a raw rusqlite
  "no such table" panic. Without these two tests, the
  explicit-bootstrap contract is unfalsifiable — an implementation
  that quietly bootstraps in `open()` would still pass every test
  the plan currently lists.
- [N8] [D5] (no pragma tuning in the wrapper) and [D6] (file-backed
  multi-connection interop tests) together leave the test setup
  unspecified. Default rusqlite uses the rollback journal with no
  `busy_timeout`; a wrapper claim contending with a raw-core claim
  on the same file under those defaults will hit `SQLITE_BUSY`
  immediately. Phase 001's multi-connection tests must have set
  pragmas in test setup. The plan should say where the *interop*
  tests get their connection settings from now that neither the
  wrapper nor the core touches pragmas. Otherwise [D6]'s test
  matrix is non-flaky in description and flaky in execution.
- [N9] [D7] says "keep the wrapper intentionally small and say so
  plainly," but the plan revisions did the opposite: the API
  surface grew from "open + four verbs" to "open + bootstrap +
  as_ref + four verbs + four `*_at` verbs" on each of two wrapper
  types. The "say so plainly" half of [D7] is not visible anywhere
  in the plan goal, outcome, or risks. Either the plan should
  trim, or it should acknowledge that the response to round 1
  grew the deliverable on purpose.
- [N10] `BouncerRef::new(conn: &rusqlite::Connection) -> Result<Self>`
  returns a `Result` even though [D1] moved bootstrap out of
  construction. With nothing fallible left in the borrowed
  constructor, the `Result` is dead weight and may quietly invite
  the implementer to put schema-checking back into the
  constructor — re-introducing exactly the side effect [A2]
  warned against. Probably should be infallible.

### Adversarial review

- [A6] After [D2] and [D3] there are now five paths to a claim:
  `bouncer-honker::claim`, `Bouncer::claim`, `Bouncer::claim_at`,
  `BouncerRef::claim`, and `BouncerRef::claim_at`. The four wrapper
  variants are each one line of delegation. What does
  `Bouncer::claim_at(name, owner, now_ms, ttl_ms)` add over calling
  `bouncer_honker::claim(&conn, name, owner, now_ms, ttl_ms)`
  directly? If the honest answer is "method dispatch sugar on a
  struct," the plan should admit it. If the honest answer is
  "nothing," `*_at` should be deferred to "drop down to core for
  deterministic tests." Either way, four near-identical variants
  per type is loud for a phase whose stated goal is "thin."
- [A7] [D8]'s premise — that the Rust wrapper shape will template
  future TS/Python bindings — is partly undermined by [D3]. TS and
  Python don't share Rust's explicit-time idiom; idiomatic
  bindings in those languages will not have `claim_at`-style
  methods on each wrapper. If `*_at` is going to evaporate at the
  language boundary anyway, then the Phase 002 surface is not
  actually the cross-language template the plan now claims it is.
  The actually-load-bearing decisions for future bindings are
  [D1] (explicit bootstrap), [D2] (owned vs borrowed), and [D4]
  (re-exported result shapes), and the plan should say so.
- [A8] The decisions accepted every Round 1 finding (none deferred,
  none rejected). That is suspicious. [N5] in particular argued
  for *more* commitment around pragmas, and [D5] resolved it by
  explicitly committing to *no* pragma policy — which is a real
  decision but it is not the same as fixing the underlying
  concern, since the concern was "the wrapper might silently
  break Phase 001's contention guarantees." Pushing that policy
  to the caller answers "who" but not "do the contention
  guarantees still hold under the test matrix [D6] requires." See
  [N8].
- [A9] The decisions grew the public shape, the test matrix, and
  the file count without trimming anything. That is a normal
  response to a critical first review, but it does mean Phase 002
  is now meaningfully larger than the spec-diff phrase "first
  thin Rust binding" implies. A future reviewer reading only
  `spec-diff.md` will under-estimate the phase's actual scope. The
  plain-English half of [D7] should land in the spec diff, not
  just be promised as a tone in implementation.

### Review verdict

- Accepted with minor follow-up before coding.

The decisions resolve the Round 1 findings directionally, and most
([D1], [D2], [D4], [D5], [D6]) land in the artifacts cleanly. Three
items should be addressed before implementation:

1. Add the negative bootstrap tests called out in [N7], so the
   explicit-bootstrap contract is actually testable.
2. Specify where the interop tests get their SQLite connection
   settings from, given that neither the wrapper nor the core
   manages pragmas after [D5] ([N8]).
3. Either trim the `*_at` surface or admit plainly that Phase 002
   has grown beyond "thin" ([N9], [A6], [A9]).

[N10] is small and can be fixed in implementation. [A7] is a future
note for whoever scopes the first non-Rust binding — not blocking
for Phase 002.

## Decision Round 002

Responding to:
- Review Round 002
- post-review discussion about how the wrapper's system-time path
  should actually read the clock (the unixepoch() vs.
  `SystemTime::now()` thread)

Session:
- A

### Inputs

- [P6]
- [P7]
- [P8]
- [P9]
- [P10]
- [N7]
- [N8]
- [N9]
- [N10]
- [A6]
- [A7]
- [A8]
- [A9]
- discussion: failure modes of `now_ms` under high throughput,
  multi-process clock skew, and the call-site-vs-commit-time gap; the
  observation that `unixepoch('subsec') * 1000` read inside the
  `BEGIN IMMEDIATE` transaction collapses that gap and is
  process-independent on a single host

### Decisions

- [D9] New direction: pull the SQL-side time-read into Phase 002.
  Action: `bouncer-honker` grows `inspect_now`, `claim_now`,
  `renew_now`, and `release_now` that read
  `unixepoch('subsec') * 1000` inside the existing `BEGIN IMMEDIATE`
  transactions. Phase 001 lease semantics do not change; only the
  time source differs. The Phase 002 wrapper's system-time methods
  delegate to those `_now` variants instead of calling
  `SystemTime::now()` in Rust. The wrapper's `*_at` methods
  delegate to the existing explicit-time variants unchanged.
  Rationale: closes the call-site-vs-commit-time gap (which would
  silently issue already-expired short-TTL leases under contention),
  makes the system-time path process-independent on a single host
  (kernel clock is now the only source), and keeps the wrapper
  one-line-per-method. The Rust-side clock-read becomes dead code
  Phase 002 never had to write. This direction was raised and
  agreed in the post-Round-2 discussion, not in the original
  Round-1 plan.
  Targets: `spec-diff.md`, `plan.md`, `bouncer-honker` scope

- [D10] Defer the broader DST-forward time contract to a future
  phase.
  Action: capture the proposal in `ROADMAP.md` under "Future
  proposals". The proposal commits to a stance ("stored expiry
  wins, with a soft `Error::ClockWentBackward` guard at a 5s
  tolerance"), six DST property tests using the explicit `*_at`
  API, and a new "Time and clocks" section in `SYSTEM.md`. The
  primitive (the soft guard and the new error variant) lives in
  honker so future siblings can opt in.
  Rationale: DST-forwardness is a meaningful contract change, not
  a binding feature. Bundling it into Phase 002 would muddy what
  this phase actually proves. This punt was raised and agreed in
  the post-Round-2 discussion.
  Targets: `ROADMAP.md`, future phase

- [D11] Round 2 findings [N7], [N8], [N9], [N10], [A6], [A7], and
  [A9] remain open.
  Action: address in a follow-up decision round before
  implementation begins. [A8] is observational and does not need an
  action by itself; [N5] is genuinely resolved by [D5] in the sense
  the plan intended (caller owns pragma policy), but the contention
  question [A8] raises is real and lands as part of [N8].
  Rationale: this decision round is scoped to the time-source
  change and the DST roadmap punt. The remaining structural
  findings (negative bootstrap tests, interop pragma setup, surface
  trimming, the dead `Result` on `BouncerRef::new`, and the
  `*_at`-everywhere question) deserve their own pass and should not
  be rushed.
  Targets: next decision round

### Verification

- `spec-diff.md` now lists the `bouncer-honker` `_now` variants and
  states that the wrapper's system-time path delegates to them; the
  verification block adds the `_now` parity check and the
  "wrapper does not call `SystemTime::now()`" assertion
- `plan.md` adds `_now` variants to the phase outcome, restates the
  time-handling section around the SQL-side read, inserts a new
  build-order step (step 3) for the `_now` variants, renumbers the
  later steps, lists `bouncer-honker/src/lib.rs` under files likely
  to change, and explicitly carves the DST-forward contract out of
  scope with a pointer to `ROADMAP.md`
- `ROADMAP.md` carries the "DST-forward time contract" proposal
  under a new "Future proposals" section, including the stance, the
  test matrix, and the explicit out-of-scope items
- Round 2 findings still open are listed in [D11] so the next
  decision round has an explicit punch list

### Decision verdict

- Time-source decision locked. Remaining Round 2 findings still
  open and require a follow-up decision round before coding begins.

## Decision Round 003

Responding to:
- Decision Round 002 (specifically [D9] and [D10])
- a clarification surfaced in the post-Round-2 discussion: the
  shorthand "DST" used to motivate the future-proposal punt meant
  **deterministic simulation testing** (TigerBeetle/FoundationDB/sled
  style), not Daylight Saving Time / wall-clock-jump handling.
  Decision Round 002 was written under the wrong reading of "DST"
  and committed the project to the wrong direction.

Session:
- A

### Inputs

- [D9] (now reversed)
- [D10] (now reversed)
- [D11] (still applies)
- the corrected reading: "DST-forward" = "deterministic simulation
  testable" = every source of non-determinism (clock, schedule,
  randomness, I/O, failure) flows through a seam the test harness
  controls

### Decisions

- [D12] Reverse [D9].
  Action: do not add `_now` variants to `bouncer-honker`. Do not
  read `unixepoch()` inside `BEGIN IMMEDIATE`. The wrapper's
  system-time path stays in Rust (`SystemTime::now()` or, better,
  a `Clock` trait with a system-time default) so the test harness
  can swap it.
  Rationale: a SQL-side clock read is anti-DST. It pulls a
  non-determinism source (the kernel clock) into the lowest layer,
  past every seam the simulation harness might install. The
  call-site-vs-commit-time gap that motivated [D9] is real in
  production, but DST handles it differently — by compressing or
  controlling time in tests via virtual clocks, not by closing the
  gap with a hidden time source.
  Targets: `spec-diff.md` (reverted), `plan.md` (reverted),
  `bouncer-honker` scope (no changes from Phase 002)

- [D13] Reverse [D10].
  Action: replace the "DST-forward time contract" roadmap entry
  (which described Daylight Saving Time / wall-clock-jump
  defenses) with a "DST-forward (deterministic simulation
  testing)" entry covering clock seam, op generator, scheduler,
  VFS shim, property runner. Honker hosts the harness; siblings
  inherit it.
  Rationale: the previous roadmap entry was solving the wrong
  problem. Wall-clock-jump defenses (soft `ClockWentBackward`
  guard, six DST-named property tests) are not what was asked
  for and are not where the value is for this project family.
  Targets: `ROADMAP.md`

- [D14] Note for the next decision round: a future Phase 002
  decision (or a follow-on phase) will need to commit to whether
  the wrapper's `Clock` seam is a trait, a function pointer, or
  just "production = `SystemTime::now()`, tests = `*_at`". This
  is now a deterministic-simulation concern, not a time-correctness
  concern. Track it alongside [D11]'s open Round 2 findings.
  Targets: next decision round

- [D15] [D11] still applies. Round 2 findings [N7], [N8], [N9],
  [N10], [A6], [A7], and [A9] remain open and need their own
  decision round before coding begins. The reversal of [D9]/[D10]
  does not change them.
  Targets: next decision round

### Verification

- `spec-diff.md` reverted: `_now` variant bullet removed from
  "What changes," `_now` parity and "no `SystemTime::now()` in
  Rust" assertions removed from "How we will verify it"
- `plan.md` reverted: `_now` variants removed from phase outcome,
  Phase decisions back to "system time by default" + `*_at`
  variants, time-handling section back to its pre-[D9] form,
  build-order step 3 returned to "thin method delegation" with
  later steps renumbered down, `bouncer-honker/src/lib.rs`
  removed from files-likely-to-change, "stronger clock-jump
  contract" removed from areas-not-touched
- `ROADMAP.md` "Future proposals" section now describes
  deterministic simulation testing (clock seam, op generator,
  scheduler, VFS shim, property runner) and explicitly notes
  that SQL-side time reads (the [D9] direction) are inconsistent
  with this future direction
- this round records the misread plainly so the history shows why
  [D9] and [D10] were reversed rather than implying they were
  wrong on the merits

### Decision verdict

- [D9] and [D10] reversed. Phase 002 spec-diff and plan are back
  to their post-Decision-Round-001 state. The deterministic
  simulation roadmap entry replaces the Daylight Saving entry.
  Round 2 findings still open per [D11]/[D15].

## Review Round 003

Target:
- implementation review

Session:
- B

Model family:
- Claude Opus 4.7

Artifacts reviewed:
- `spec-diff.md` (current)
- `plan.md` (current)
- `packages/bouncer/Cargo.toml`
- `packages/bouncer/src/lib.rs` (implementation + tests)
- `packages/bouncer/README.md`
- workspace `Cargo.toml`
- `commits.txt`

Verification reviewed:
- `cargo test` from repo root: 8 wrapper tests pass, 16 core
  tests still pass, 0 doctests run

### Positive conformance review

- [P11] The implementation maps cleanly to the seven verification
  items in the current `spec-diff.md`. Each has a named test:
  full lease cycle, pre-bootstrap clean failure (twice — for
  `claim` and `inspect`), wrapper↔core interop in both
  directions, separate-connection interop, fencing-token
  monotonicity across wrapper/core, and bootstrap idempotence.
  Nothing in the spec-diff verification list is missing a test.
- [P12] `BouncerRef::new(conn) -> Self` is infallible. This
  closes the [N10] hole from Round 2 — there's no dead `Result`
  inviting a future implementer to put schema-checking back into
  the constructor.
- [P13] Bootstrap is genuinely explicit. `Bouncer::open()` opens
  a plain rusqlite connection and does nothing schema-related.
  The two negative tests
  (`open_does_not_bootstrap_implicitly`,
  `wrapper_methods_fail_cleanly_before_bootstrap`) actively
  prove the property — closing the [N7] hole that Round 2 said
  was the most important blocker.
- [P14] Test-harness pragma setup
  (`configure_test_connection` with WAL + 1s `busy_timeout`)
  exists, is named clearly as harness configuration, and is
  applied to both the wrapper-owned connection and the
  side-channel core connection in the interop tests. The plan's
  step-5 commitment ("if test setup needs connection settings
  for reliability, put them in a test-only helper and document
  them as harness configuration rather than product behavior")
  is honored exactly. Closes [N8].
- [P15] No `.unwrap()` outside test code. `system_now_ms()`
  propagates `SystemTimeError` and the `u128 → i64` conversion
  is explicit via `try_from`. Matches the project's fail-fast
  pattern.
- [P16] The wrapper stayed honestly thin: ~110 lines of
  non-test code. `Bouncer` lease verbs are one-line delegations
  to `self.as_ref().<verb>(...)`, and `BouncerRef` lease verbs
  are one-line delegations to `core::<verb>(...)`. The
  implementation does not invent a parallel state machine.
  This effectively closes [N9], [A6], [A9] from Round 2 — the
  surface trim happened by removing `*_at` variants from the
  plan rather than by writing them and then deleting them.
- [P17] Result-shape re-export
  (`pub use core::{ClaimResult, LeaseInfo, ReleaseResult,
  RenewResult}`) matches [D4] cleanly. Callers don't need a
  direct `bouncer-honker` dep to pattern-match results.

### Negative conformance review

- [N11] The negative-bootstrap tests use
  `core_missing_schema_error` which matches
  `Error::Core(core::Error::Sqlite(_))` — that is, **any**
  rusqlite error wrapped in a core error. If a future change
  causes `claim`/`inspect` to fail with a different rusqlite
  error in this position (e.g. `database is locked` or
  `disk I/O error`), the negative-bootstrap tests would pass
  for the wrong reason. Tighten by matching on the rusqlite
  `ErrorCode` or on the error message containing `no such
  table`.
- [N12] `Bouncer::as_ref(&self) -> BouncerRef<'_>` is an
  inherent method that shadows the `AsRef` trait method name.
  Likely trips `clippy::should_implement_trait` and is mildly
  confusing in generic contexts. Rename to `borrow()` or
  `as_bouncer_ref()`, or implement an actual trait. Cheap to
  fix while the surface has zero external callers.
- [N13] `Error::SystemTimeTooLarge(u128)` and
  `Error::DurationTooLarge(Duration)` defend against
  year-292-million overflows. They are honest fail-fast paths,
  but they imply normal-condition fallibility that the call
  sites never actually exercise. Either keep with a one-line
  comment explaining "these are unreachable on any sane
  wallclock," or downgrade to `expect()`. Current shape adds
  call-site noise without buying real safety.
- [N14] The spec-diff line "The binding does not use wall
  clock as an ordering primitive" is not directly tested. The
  fencing-token-monotonicity test demonstrates that fencing
  *works*, but doesn't demonstrate that the wrapper falls back
  to fencing rather than wallclock when those would disagree.
  An affirmative test would use `bouncer-honker` directly with
  deliberately-skewed `now_ms` values and assert that fencing
  catches a stale actor that wallclock comparison would miss.
  Optional but tightens the contract.

### Adversarial review

- [A10] `Bouncer::open()` opens a connection with rusqlite
  defaults — no `journal_mode`, no `busy_timeout`. The README
  and the plan correctly punt connection policy to the caller.
  But the README example
  (`Bouncer::open("app.sqlite3")?; db.bootstrap()?; ...`)
  doesn't mention this. Anyone copy-pasting the example into a
  multi-process app will hit `SQLITE_BUSY` on the first
  contention. A one-line note in the README ("if multiple
  connections will touch this file, set `journal_mode=WAL` and
  a `busy_timeout` before calling `bootstrap()`") would
  prevent the most likely first-real-use foot-gun. Not blocking
  — it's a doc concern, not a code concern — but the README is
  the place a new caller looks first.
- [A11] The 8 tests cover correctness in deterministic
  sequences but not contention. The
  `fencing_token_stays_monotonic_across_wrapper_and_core` test
  takes the second claim only after computing
  `first.lease_expires_at_ms + 1` — i.e. it sequences the two
  claims so they cannot race. Phase 001 covered file-backed
  multi-connection contention; Phase 002 inherits that
  coverage at the core but doesn't re-prove it through the
  wrapper. Defensible scope (the wrapper is delegation, not
  new logic), but worth stating plainly in the phase summary
  rather than implying contention is freshly proven.
- [A12] The implementation looks done because all the things
  Round 1 and Round 2 worried about are resolved. That is true
  on the explicit checklist, but two are resolved by *removal*
  rather than by *answer*: `*_at` variants were deleted (not
  designed correctly), and the wall-clock jump contract was
  punted to a deterministic-simulation roadmap entry (not
  resolved at the wrapper layer). That's the right call for
  Phase 002's scope, but anyone reading only this phase's
  artifacts later might think "wrapper is complete" when the
  honest framing is "wrapper is complete *for the things
  Phase 002 chose to commit to*." The plain-English half of
  [D7] should land in `SYSTEM.md` when Phase 002 closes —
  specifically, that the wrapper's safety story for stale
  actors flows through fencing tokens at the application
  layer, not through clock arithmetic at the lease layer.

### Review verdict

- Accepted with two cheap polish items before SYSTEM.md
  update.

Implementation closes every Round 1 negative finding ([N1]–[N6])
and every Round 2 negative finding ([N7]–[N10]). [A6], [A7], [A9]
from Round 2 are also resolved by the simpler shape that landed.
The two polish items worth fixing before this phase closes:

1. Tighten `core_missing_schema_error` ([N11]) — the negative
   bootstrap tests are currently passing for a slightly weaker
   reason than they should.
2. Rename `Bouncer::as_ref` to avoid the `AsRef` trait shadow
   ([N12]).

[N13] (defensive error variants), [N14] (affirmative
no-wallclock-ordering test), [A10] (README note about
connection policy in multi-process apps), [A11] (note about
contention coverage scope), and [A12] (SYSTEM.md framing on
Phase 002 close) are nice-to-haves and can land alongside or
after the SYSTEM.md update. Implementation is shippable as is;
these would just leave the phase tighter for the next reader.

## Decision Round 004

Responding to:
- Review Round 002
- [D11]
- [D15]
- follow-up discussion about whether milliseconds need to order
  high-throughput wrapper writes

Session:
- A

### Inputs

- [N7]
- [N8]
- [N9]
- [N10]
- [A6]
- [A7]
- [A8]
- [A9]
- discussion: lease timestamps are expiry bookkeeping, not an ordering
  primitive; ordering and stale-actor safety come from SQLite writer
  serialization plus the fencing token

### Decisions

- [D16] Accept [N7]
  Action: make the negative bootstrap contract explicit in the phase
  artifacts. `open(path)` must not bootstrap implicitly, and wrapper
  methods before `bootstrap()` must fail cleanly.
  Targets: `spec-diff.md`, `plan.md`, tests

- [D17] Accept [N8] and [A8]
  Action: clarify that Phase 002 interop tests are file-backed,
  separate-connection interoperability tests, not a concurrency-policy
  contract. Wrapper/core stay pragma-neutral. If a test helper needs
  connection settings for reliability, that lives in the test harness,
  not in product behavior.
  Targets: `plan.md`, tests

- [D18] Accept [N9], [A6], and [A9]
  Action: trim the wrapper public surface back down. Remove wrapper
  `*_at` variants from Phase 002. The thin wrapper is now
  `open/bootstrap/as_ref/new` plus the four lease verbs only.
  Rationale: milliseconds do not need to order wrapper writes. Time is
  for expiry. Ordering and stale-actor safety come from SQLite writer
  serialization and fencing tokens. Exposing `*_at` across both wrapper
  types made the phase louder without improving the correctness story.
  Explicit-time control remains in `bouncer-honker`.
  Targets: `spec-diff.md`, `plan.md`

- [D19] Accept [N10]
  Action: make `BouncerRef::new(&Connection)` infallible.
  Targets: `plan.md`, implementation shape

- [D20] Accept [A7]
  Action: say more plainly that the Rust wrapper is not the exact
  future-language template. The cross-language commitments from Phase 002
  are explicit bootstrap, thin delegation, and result-shape reuse.
  Targets: `plan.md`, docs language

- [D21] Partially accept [A6] and [A7]; defer wrapper-level clock seam.
  Action: keep Phase 002 small and system-time-backed. If deterministic
  simulation later needs a wrapper `Clock` seam, make that its own
  decision or follow-on phase rather than smuggling it into this one.
  Targets: `plan.md`, `ROADMAP.md`

### Verification

- `spec-diff.md` now says wrapper convenience methods use system-time
  defaults while explicit-time control remains in `bouncer-honker`.
- `plan.md` removes wrapper `*_at` methods, makes `BouncerRef::new`
  infallible, adds the negative bootstrap tests, and clarifies that
  pragma setup for interop tests belongs to test harness configuration
  rather than product behavior.
- `ROADMAP.md` now frames a wrapper clock seam as a future possibility
  rather than a Phase 002 requirement.

### Decision verdict

- Round 2 findings are materially resolved for Phase 002 planning.
- Phase 002 stays thin again.
- The remaining time-ordering concern is answered at the model level:
  timestamps are for expiry, not ordering.

## Decision Round 005

Responding to:
- Review Round 003

Session:
- A

### Inputs

- [P11]
- [P12]
- [P13]
- [P14]
- [P15]
- [P16]
- [P17]
- [N11]
- [N12]
- [N13]
- [N14]
- [A10]
- [A11]
- [A12]

### Decisions

- [D22] Accept [N11]
  Action: tighten the negative-bootstrap matcher so the tests prove the
  missing-schema path specifically, not just any wrapped rusqlite error.
  Targets: wrapper tests

- [D23] Accept [N12]
  Action: rename `Bouncer::as_ref` to `Bouncer::borrowed` to avoid
  shadowing the `AsRef` trait shape.
  Targets: wrapper API

- [D24] Defer [N13]
  Action: keep the defensive overflow variants for now. They are noisy
  but honest, and they are not phase-blocking.
  Targets: future cleanup

- [D25] Defer [N14]
  Action: leave the affirmative no-wallclock-ordering proof for a later
  phase or a deeper core/property test pass. Phase 002's contract is
  already stated in the artifacts, and the current wrapper does not add
  any new ordering logic of its own.
  Targets: future tests

- [D26] Accept [A10]
  Action: add a README note that multi-process callers should set
  connection policy such as `journal_mode=WAL` and `busy_timeout`
  themselves before bootstrap.
  Targets: package README

- [D27] Accept [A11] and [A12]
  Action: say plainly in `SYSTEM.md` that wrapper tests prove thin
  delegation and wrapper/core interop, while contention semantics and
  stale-actor safety still primarily live at the core/fencing layer.
  Targets: `SYSTEM.md`

### Verification

- wrapper tests now match the missing-schema error more specifically
- `Bouncer::borrowed()` replaces `Bouncer::as_ref()`
- package README now notes pragma policy for multi-process callers
- `SYSTEM.md` now describes the Phase 002 baseline honestly, including
  the wrapper's safety story and scope limits

### Decision verdict

- Phase 002 is accepted as the current baseline after the cheap polish
  items.
