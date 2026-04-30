from __future__ import annotations

import bouncer


def main() -> None:
    db = bouncer.open("app.sqlite3")
    db.bootstrap()

    result = db.claim("scheduler", "worker-a", ttl_ms=30_000)
    if result.acquired:
        print(f"acquired token {result.lease.token}")
    else:
        print(f"busy: currently owned by {result.current.owner}")


if __name__ == "__main__":
    main()
