//! Phase 011 deterministic invariant runner for `bouncer-core`.
//!
//! Generates seeded sequences of explicit-time lease operations against an
//! in-memory SQLite database and asserts core lease invariants after every
//! step. Failures print the seed and step so any finding is replayable.
//!
//! Scope is pinned by the phase plan:
//!
//! - core-only, in-memory SQLite, default pragmas
//! - public `bouncer-core` API for lease behavior; direct table reads only
//!   for row-shape checks
//! - autocommit `claim`/`renew`/`release` only; `*_in_tx` belongs to the
//!   later SQLite behavior matrix phase
//! - mutation operations advance a sequence-monotonic model clock; read
//!   operations may sample non-monotonic times around lease boundaries
//! - 1000 seeds x 100 steps for the default generated test
//! - 4 resources x 6 owners

use bouncer_core::{
    bootstrap_bouncer_schema, claim, inspect, owner as core_owner, release, renew, token,
    ClaimResult, LeaseInfo, ReleaseResult, RenewResult,
};
use rusqlite::{params, Connection, OptionalExtension};

const RESOURCES: &[&str] = &["scheduler", "janitor", "ingester", "rotator"];
const OWNERS: &[&str] = &[
    "worker-a",
    "worker-b",
    "worker-c",
    "worker-d",
    "worker-e",
    "worker-f",
];

#[derive(Debug, Clone, Copy)]
enum Op {
    Claim {
        resource: usize,
        owner: usize,
        ttl_ms: i64,
    },
    Renew {
        resource: usize,
        owner: usize,
        ttl_ms: i64,
    },
    Release {
        resource: usize,
        owner: usize,
    },
    Inspect {
        resource: usize,
    },
    Owner {
        resource: usize,
    },
    Token {
        resource: usize,
    },
    AdvanceTime {
        delta_ms: i64,
    },
}

/// xorshift64-style deterministic RNG. Tiny on purpose; the runner only needs
/// reproducible choices, not statistical quality.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        // xorshift64 collapses to zero if the state ever reaches zero, and
        // mixing with a non-zero offset also makes adjacent seeds diverge
        // quickly.
        Rng(seed.wrapping_add(0x9E37_79B9_7F4A_7C15) | 1)
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    fn range_usize(&mut self, max: usize) -> usize {
        (self.next_u64() % (max as u64)) as usize
    }

    fn range_i64_inclusive(&mut self, lo: i64, hi: i64) -> i64 {
        let span = (hi - lo + 1) as u64;
        lo + (self.next_u64() % span) as i64
    }
}

#[derive(Debug, Clone)]
struct ModelResource {
    /// 0 means "no row has ever been written for this resource". Any
    /// positive value is the highest fencing token seen.
    last_token: i64,
    /// Mirrors the row's `owner` column. Set on successful claim, cleared
    /// on successful release. Persists across expiry.
    row_owner: Option<String>,
    /// Mirrors the row's `lease_expires_at_ms` column. Set on successful
    /// claim/renew, cleared on successful release. Persists across expiry.
    row_expires_at_ms: Option<i64>,
}

impl ModelResource {
    fn new() -> Self {
        Self {
            last_token: 0,
            row_owner: None,
            row_expires_at_ms: None,
        }
    }

    fn current_lease(&self, name: &str, now_ms: i64) -> Option<LeaseInfo> {
        match (&self.row_owner, self.row_expires_at_ms) {
            (Some(owner), Some(exp)) if exp > now_ms => Some(LeaseInfo {
                name: name.to_owned(),
                owner: owner.clone(),
                token: self.last_token,
                lease_expires_at_ms: exp,
            }),
            _ => None,
        }
    }
}

struct Model {
    resources: Vec<ModelResource>,
    /// Sequence-monotonic clock. Mutators advance it by 1 before applying
    /// their now_ms; AdvanceTime adds further deltas. Reads do not move it.
    now_ms: i64,
}

impl Model {
    fn new() -> Self {
        Self {
            resources: (0..RESOURCES.len()).map(|_| ModelResource::new()).collect(),
            now_ms: 1,
        }
    }
}

fn open_db() -> Connection {
    let conn = Connection::open_in_memory().expect("open in-memory sqlite");
    bootstrap_bouncer_schema(&conn).expect("bootstrap schema");
    conn
}

/// Read the raw row for `name`. Used only for row-shape invariants; the
/// public API still owns lease semantics.
fn read_row(conn: &Connection, name: &str) -> Option<(Option<String>, i64, Option<i64>)> {
    conn.query_row(
        "SELECT owner, token, lease_expires_at_ms FROM bouncer_resources WHERE name = ?1",
        params![name],
        |row| {
            let owner: Option<String> = row.get(0)?;
            let tok: i64 = row.get(1)?;
            let expires: Option<i64> = row.get(2)?;
            Ok((owner, tok, expires))
        },
    )
    .optional()
    .expect("read bouncer_resources row")
}

/// Times to sample for read operations around the resource's lease boundary.
/// Always includes the model clock; if the resource has a stored expiry,
/// also samples just before, at, and just after that boundary.
fn boundary_sample_times(model: &Model, resource: usize) -> Vec<i64> {
    let mut times = vec![model.now_ms];
    if let Some(exp) = model.resources[resource].row_expires_at_ms {
        times.push(exp.saturating_sub(1));
        times.push(exp);
        times.push(exp.saturating_add(1));
    }
    times
}

fn apply_step(conn: &Connection, model: &mut Model, op: Op, seed: u64, step: usize) {
    match op {
        Op::Claim {
            resource,
            owner: oidx,
            ttl_ms,
        } => {
            model.now_ms = model
                .now_ms
                .checked_add(1)
                .expect("model now_ms overflow on claim");
            let now_ms = model.now_ms;
            let res_name = RESOURCES[resource];
            let owner_name = OWNERS[oidx];

            let prior_token = model.resources[resource].last_token;
            let prior_live = model.resources[resource].current_lease(res_name, now_ms);

            let result = claim(conn, res_name, owner_name, now_ms, ttl_ms)
                .unwrap_or_else(|e| panic!("seed={seed} step={step} claim error: {e:?}"));

            match (result, prior_live) {
                (ClaimResult::Acquired(lease), None) => {
                    let expected_token = if prior_token == 0 { 1 } else { prior_token + 1 };
                    let exp = now_ms + ttl_ms;
                    assert_eq!(
                        lease,
                        LeaseInfo {
                            name: res_name.to_owned(),
                            owner: owner_name.to_owned(),
                            token: expected_token,
                            lease_expires_at_ms: exp,
                        },
                        "seed={seed} step={step} acquired lease shape mismatch"
                    );
                    model.resources[resource].last_token = expected_token;
                    model.resources[resource].row_owner = Some(owner_name.to_owned());
                    model.resources[resource].row_expires_at_ms = Some(exp);
                }
                (ClaimResult::Busy(current), Some(expected)) => {
                    assert_eq!(
                        current, expected,
                        "seed={seed} step={step} busy lease should match prior live lease"
                    );
                }
                (other, expected_prior_live) => panic!(
                    "seed={seed} step={step} claim result/model disagree: \
                     result={other:?} expected_prior_live={expected_prior_live:?}"
                ),
            }
        }
        Op::Renew {
            resource,
            owner: oidx,
            ttl_ms,
        } => {
            model.now_ms = model
                .now_ms
                .checked_add(1)
                .expect("model now_ms overflow on renew");
            let now_ms = model.now_ms;
            let res_name = RESOURCES[resource];
            let owner_name = OWNERS[oidx];
            let prior_live = model.resources[resource].current_lease(res_name, now_ms);

            let result = renew(conn, res_name, owner_name, now_ms, ttl_ms)
                .unwrap_or_else(|e| panic!("seed={seed} step={step} renew error: {e:?}"));

            match (result, prior_live) {
                (RenewResult::Renewed(lease), Some(prev)) if prev.owner == owner_name => {
                    let requested_exp = now_ms + ttl_ms;
                    let exp = prev.lease_expires_at_ms.max(requested_exp);
                    assert_eq!(
                        lease.token, prev.token,
                        "seed={seed} step={step} renew should not change token"
                    );
                    assert_eq!(
                        lease.owner, prev.owner,
                        "seed={seed} step={step} renew should not change owner"
                    );
                    assert_eq!(
                        lease.lease_expires_at_ms, exp,
                        "seed={seed} step={step} renew expiry should not shorten"
                    );
                    model.resources[resource].row_expires_at_ms = Some(exp);
                }
                (RenewResult::Rejected { current }, Some(prev)) if prev.owner != owner_name => {
                    assert_eq!(
                        current,
                        Some(prev),
                        "seed={seed} step={step} wrong-owner renew should report current lease"
                    );
                }
                (RenewResult::Rejected { current: None }, None) => {
                    // No live lease in either model or core; nothing to mutate.
                }
                (other, expected_prior_live) => panic!(
                    "seed={seed} step={step} renew result/model disagree: \
                     result={other:?} expected_prior_live={expected_prior_live:?}"
                ),
            }
        }
        Op::Release {
            resource,
            owner: oidx,
        } => {
            model.now_ms = model
                .now_ms
                .checked_add(1)
                .expect("model now_ms overflow on release");
            let now_ms = model.now_ms;
            let res_name = RESOURCES[resource];
            let owner_name = OWNERS[oidx];
            let prior_live = model.resources[resource].current_lease(res_name, now_ms);

            let result = release(conn, res_name, owner_name, now_ms)
                .unwrap_or_else(|e| panic!("seed={seed} step={step} release error: {e:?}"));

            match (result, prior_live) {
                (ReleaseResult::Released { name, token: t }, Some(prev))
                    if prev.owner == owner_name =>
                {
                    assert_eq!(
                        name, res_name,
                        "seed={seed} step={step} release name mismatch"
                    );
                    assert_eq!(
                        t, prev.token,
                        "seed={seed} step={step} release should report prior token"
                    );
                    model.resources[resource].row_owner = None;
                    model.resources[resource].row_expires_at_ms = None;
                    // last_token preserved across release; reclaim must
                    // strictly increase it.
                }
                (ReleaseResult::Rejected { current }, Some(prev)) if prev.owner != owner_name => {
                    assert_eq!(
                        current,
                        Some(prev),
                        "seed={seed} step={step} wrong-owner release should report current lease"
                    );
                }
                (ReleaseResult::Rejected { current: None }, None) => {
                    // No live lease in either model or core; nothing to mutate.
                }
                (other, expected_prior_live) => panic!(
                    "seed={seed} step={step} release result/model disagree: \
                     result={other:?} expected_prior_live={expected_prior_live:?}"
                ),
            }
        }
        Op::Inspect { resource } => {
            let res_name = RESOURCES[resource];
            for now_ms in boundary_sample_times(model, resource) {
                let result = inspect(conn, res_name, now_ms)
                    .unwrap_or_else(|e| panic!("seed={seed} step={step} inspect error: {e:?}"));
                let expected = model.resources[resource].current_lease(res_name, now_ms);
                assert_eq!(
                    result, expected,
                    "seed={seed} step={step} inspect mismatch at now_ms={now_ms}"
                );
            }
        }
        Op::Owner { resource } => {
            let res_name = RESOURCES[resource];
            for now_ms in boundary_sample_times(model, resource) {
                let result = core_owner(conn, res_name, now_ms)
                    .unwrap_or_else(|e| panic!("seed={seed} step={step} owner error: {e:?}"));
                let expected = model.resources[resource]
                    .current_lease(res_name, now_ms)
                    .map(|lease| lease.owner);
                assert_eq!(
                    result, expected,
                    "seed={seed} step={step} owner mismatch at now_ms={now_ms}"
                );
            }
        }
        Op::Token { resource } => {
            let res_name = RESOURCES[resource];
            let result = token(conn, res_name)
                .unwrap_or_else(|e| panic!("seed={seed} step={step} token error: {e:?}"));
            let expected = if model.resources[resource].last_token == 0 {
                None
            } else {
                Some(model.resources[resource].last_token)
            };
            assert_eq!(
                result, expected,
                "seed={seed} step={step} token mismatch"
            );
        }
        Op::AdvanceTime { delta_ms } => {
            model.now_ms = model
                .now_ms
                .checked_add(delta_ms)
                .expect("model now_ms overflow on AdvanceTime");
        }
    }

    check_invariants(conn, model, seed, step);
}

/// Universal invariants checked after every step, for every resource.
fn check_invariants(conn: &Connection, model: &Model, seed: u64, step: usize) {
    let now_ms = model.now_ms;

    for (idx, name) in RESOURCES.iter().enumerate() {
        let model_res = &model.resources[idx];

        let inspected = inspect(conn, name, now_ms).unwrap_or_else(|e| {
            panic!("seed={seed} step={step} invariant inspect error for {name}: {e:?}")
        });
        let owned = core_owner(conn, name, now_ms).unwrap_or_else(|e| {
            panic!("seed={seed} step={step} invariant owner error for {name}: {e:?}")
        });
        let tok = token(conn, name).unwrap_or_else(|e| {
            panic!("seed={seed} step={step} invariant token error for {name}: {e:?}")
        });

        let inspected_owner = inspected.as_ref().map(|lease| lease.owner.clone());
        assert_eq!(
            inspected_owner, owned,
            "seed={seed} step={step} resource={name} inspect/owner disagree at now_ms={now_ms}"
        );

        let expected_lease = model_res.current_lease(name, now_ms);
        assert_eq!(
            inspected, expected_lease,
            "seed={seed} step={step} resource={name} inspect/model disagree at now_ms={now_ms}"
        );

        let expected_token = if model_res.last_token == 0 {
            None
        } else {
            Some(model_res.last_token)
        };
        assert_eq!(
            tok, expected_token,
            "seed={seed} step={step} resource={name} token/model disagree"
        );

        if let Some(lease) = inspected.as_ref() {
            assert_eq!(
                Some(lease.token),
                tok,
                "seed={seed} step={step} resource={name} live lease token disagrees with token()"
            );
        }

        match read_row(conn, name) {
            None => assert_eq!(
                model_res.last_token, 0,
                "seed={seed} step={step} resource={name} \
                 row missing but model has last_token={}",
                model_res.last_token
            ),
            Some((row_owner, row_token, row_expires)) => {
                assert_eq!(
                    row_token, model_res.last_token,
                    "seed={seed} step={step} resource={name} row token mismatch"
                );
                assert_eq!(
                    row_owner, model_res.row_owner,
                    "seed={seed} step={step} resource={name} row owner mismatch"
                );
                assert_eq!(
                    row_expires, model_res.row_expires_at_ms,
                    "seed={seed} step={step} resource={name} row expires mismatch"
                );
                // Released-row invariant: owner NULL iff expires NULL.
                assert_eq!(
                    row_owner.is_none(),
                    row_expires.is_none(),
                    "seed={seed} step={step} resource={name} \
                     owner/expires nullability mismatch"
                );
            }
        }
    }
}

fn generate_op(rng: &mut Rng) -> Op {
    let bucket = rng.range_usize(100);
    let resource = rng.range_usize(RESOURCES.len());
    let owner = rng.range_usize(OWNERS.len());
    match bucket {
        0..=29 => Op::Claim {
            resource,
            owner,
            ttl_ms: rng.range_i64_inclusive(1, 12),
        },
        30..=49 => Op::Renew {
            resource,
            owner,
            ttl_ms: rng.range_i64_inclusive(1, 12),
        },
        50..=64 => Op::Release { resource, owner },
        65..=74 => Op::Inspect { resource },
        75..=79 => Op::Owner { resource },
        80..=84 => Op::Token { resource },
        _ => Op::AdvanceTime {
            delta_ms: rng.range_i64_inclusive(1, 10),
        },
    }
}

#[test]
fn fixed_sequence_exercises_full_lifecycle() {
    let conn = open_db();
    let mut model = Model::new();
    let seed: u64 = 0;

    // Hand-written sequence; one named transition per step, easy to read.
    let ops = [
        // first claim against fresh resource: token 1
        Op::Claim {
            resource: 0,
            owner: 0,
            ttl_ms: 50,
        },
        Op::Inspect { resource: 0 },
        Op::Token { resource: 0 },
        // second claim while live: busy
        Op::Claim {
            resource: 0,
            owner: 1,
            ttl_ms: 30,
        },
        // wrong-owner renew rejected, current lease unchanged
        Op::Renew {
            resource: 0,
            owner: 1,
            ttl_ms: 30,
        },
        // current owner renew succeeds, token unchanged
        Op::Renew {
            resource: 0,
            owner: 0,
            ttl_ms: 60,
        },
        // wrong-owner release rejected
        Op::Release {
            resource: 0,
            owner: 1,
        },
        // current owner release succeeds, token preserved
        Op::Release {
            resource: 0,
            owner: 0,
        },
        Op::Inspect { resource: 0 },
        Op::Token { resource: 0 },
        // reclaim after release: token strictly increases to 2
        Op::Claim {
            resource: 0,
            owner: 1,
            ttl_ms: 20,
        },
        Op::Token { resource: 0 },
        // advance past expiry
        Op::AdvanceTime { delta_ms: 100 },
        Op::Inspect { resource: 0 },
        Op::Token { resource: 0 },
        // takeover after expiry: token strictly increases to 3
        Op::Claim {
            resource: 0,
            owner: 2,
            ttl_ms: 40,
        },
        Op::Token { resource: 0 },
    ];

    for (step, op) in ops.iter().enumerate() {
        apply_step(&conn, &mut model, *op, seed, step);
    }
}

#[test]
fn generated_invariants_hold_across_seeds() {
    // Default budget pinned by the phase plan: 1000 seeds x 100 steps.
    const SEEDS: u64 = 1000;
    const STEPS: usize = 100;

    for seed in 0..SEEDS {
        let conn = open_db();
        let mut model = Model::new();
        let mut rng = Rng::new(seed);

        for step in 0..STEPS {
            let op = generate_op(&mut rng);
            apply_step(&conn, &mut model, op, seed, step);
        }
    }
}
