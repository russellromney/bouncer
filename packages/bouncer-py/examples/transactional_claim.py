from __future__ import annotations

import bouncer


def main() -> None:
    db = bouncer.open("app.sqlite3")
    db.bootstrap()

    with db.transaction() as tx:
        tx.execute(
            "CREATE TABLE IF NOT EXISTS jobs (payload TEXT NOT NULL)"
        )
        tx.execute(
            "INSERT INTO jobs(payload) VALUES (?)",
            ["run scheduler tick"],
        )

        claim = tx.claim("scheduler", "worker-a", ttl_ms=30_000)
        if claim.acquired:
            print(f"acquired token {claim.lease.token}")
        else:
            print(f"busy: currently owned by {claim.current.owner}")
            tx.rollback()


if __name__ == "__main__":
    main()
