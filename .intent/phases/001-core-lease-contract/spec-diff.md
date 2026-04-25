What changes:
- Bouncer gains its first real SQLite contract for single-machine lease ownership.
- `bouncer-honker` owns schema bootstrap plus the initial claim / renew / release / inspect operations.
- The initial contract introduces named resources, one current owner, expiry, and a fencing token.
- One thin binding is added only after the core contract exists and tests pin its behavior.

What does not change:
- Bouncer does not become distributed consensus.
- Bouncer does not add waiting queues or fairness guarantees.
- Honker does not learn Bouncer-specific concepts.
- Bouncer does not become a scheduler or workflow engine.

How we will verify it:
- A resource can be claimed by one owner and inspected from the same database.
- A second claimant cannot take the resource while the lease is still valid.
- Renew extends a valid lease for the current owner only.
- Release succeeds only for the current owner.
- Successful claim increments a fencing token monotonically.
- The contract is tested in Rust before any binding claims to expose it.
