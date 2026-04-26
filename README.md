# bouncer

Leases and coordination on top of Honker.

## Summary

Bouncer is a small wrapper library for the question:

Who owns this named resource right now?

For the Honker stack, the point is not distributed consensus or
multi-node lock choreography. The point is giving a normal SQLite app a
boring, durable lease / ownership primitive in the same file it already
uses.

It should feel like a sibling to Knocker:

- `honker` stays the generic SQLite async substrate
- `bouncer-honker` owns Bouncer's schema and SQLite operations
- `bouncer` bindings stay thin and simple

Bouncer should be the family's lease / fencing / leader-election
primitive. Over time, Honker itself may depend on it for scheduler
ownership and other single-machine coordination.

## What exists today

- repo-level docs for current intent and future direction
- a real Phase 001 `bouncer-honker` core crate
- a first Rust wrapper crate in `packages/bouncer`
- a SQLite schema bootstrap plus Rust `claim` / `renew` / `release` / `inspect`
- Rust tests that pin the initial lease semantics
- wrapper tests for explicit bootstrap and wrapper/core interoperability
- wrapper convenience methods that stay thin and keep explicit-time control in the core

## V1 shape

- named resources
- claim / renew / release
- expiry
- fencing tokens
- inspect current owner

## Target SQL surface

These are still target APIs, not implemented SQL helpers yet. Phase 001 currently exposes Rust helpers only.

- `bouncer_claim(name, owner, ttl_ms)`
- `bouncer_renew(name, owner, ttl_ms)`
- `bouncer_release(name, owner)`
- `bouncer_owner(name)`
- `bouncer_token(name)`

## Non-goals

- distributed consensus
- waiting queues
- fairness
- elaborate leader election

## Intent

Bouncer exists for the single-machine SQLite stack:

- app processes on one host
- one SQLite file
- background workers, schedulers, migrations, importers
- no Redis, Consul, ZooKeeper, or etcd just to answer "who owns this?"

If it cannot make that use case materially simpler, it should stay a
small internal primitive rather than grow into a bigger product.

## Intent artifacts

- `SYSTEM.md`
  current English model of the system
- `ROADMAP.md`
  remaining product and implementation work
- `CHANGELOG.md`
  completed work summary
- `.intent/phases/.../spec-diff.md`
  intended semantic change for one phase
- `.intent/phases/.../plan.md`
  implementation reasoning for one phase
- `.intent/phases/.../reviews_and_decisions.md`
  review history plus explicit responses for one phase

## Repo shape

- `bouncer-honker`
  Rust core that owns schema and SQLite operations
- `packages/bouncer`
  thin binding surface
