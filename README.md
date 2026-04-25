# bouncer

Leases and coordination on top of Honker.

## Summary

Bouncer is a small wrapper library for the question:

Who owns this named resource right now?

It should feel like a sibling to Knocker:

- `honker` stays the generic SQLite async substrate
- `bouncer-honker` owns Bouncer's schema and SQLite operations
- `bouncer` bindings stay thin and simple

## V1 shape

- named resources
- claim / renew / release
- expiry
- fencing tokens
- inspect current owner

## Target SQL surface

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

## Repo shape

- `bouncer-honker`
  Rust core that owns schema and SQLite operations
- `packages/bouncer`
  thin binding surface
