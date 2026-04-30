# Spec Diff

Phase: 015 — user journey acceptance

Session:
- A

## What we are building

We are adding a small acceptance layer that proves Bouncer works the
way a normal user would expect when they use the shipped public
surfaces on one real SQLite file.

This phase does not add new lease behavior. It adds better proof of the
behavior we already claim.

The new acceptance coverage should prove these user-visible journeys:

1. A user can bootstrap a fresh database and successfully take the
   first lease.
2. A second independent caller sees lease busy, not false success.
3. Releasing a lease makes the resource reclaimable, and the next
   successful claim gets a larger fencing token.
4. Expiry makes the resource reclaimable, and the next successful claim
   gets a larger fencing token.
5. The shipped public surfaces interoperate on one database file:
   - Rust wrapper
   - SQL extension
   - Python binding
6. A caller using a caller-owned transaction can combine a business
   write and a lease mutation and see both become visible together after
   commit.
7. A drifted schema fails loudly through the public bootstrap surfaces a
   user actually calls.

This phase is about proving those behaviors directly through public
APIs, not by pointing at lower-level tests and saying they are probably
enough.

## What will not change

- No new lease semantics.
- No new schema semantics.
- No new binding surface.
- No migration or repair behavior.
- No new pragma policy.
- No claim that Bouncer proves downstream stale-actor rejection outside
  SQLite. Bouncer proves token behavior; the caller still has to carry
  and compare the token at their external side-effect boundary.

## How we will prove it

We will use three proof layers, each for a different job:

### Unit / lower-level proof

Keep the existing core suites green:

- invariants
- integrity hardening
- SQLite behavior matrix
- pragma-neutrality matrix

These are supporting proof. They do not close the new user-journey
claims by themselves.

### Integration proof

Keep the existing cross-surface and wrapper integration suites green so
we know the public surfaces still share one underlying contract.

### Direct acceptance / e2e proof

Add a small acceptance suite that uses only public surfaces and one
real database file per journey.

Each acceptance journey should have one named direct-proof test.

The direct-proof surfaces are:

- Rust wrapper
- SQL extension on a raw SQLite connection
- Python binding

The direct-proof assertions should stay user-visible:

- bootstrap success or failure
- claim success versus busy
- token progression after release/reclaim or expiry/reclaim
- cross-surface visibility on one file
- business write + lease mutation visibility after commit

If one journey needs explicit time to stay deterministic, use the SQL
surface for that journey rather than adding sleeps.
