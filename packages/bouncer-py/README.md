# bouncer Python

Local-development Python binding for Bouncer.

```python
import bouncer

db = bouncer.open("app.sqlite3")
db.bootstrap()

result = db.claim("scheduler", "worker-a", ttl_ms=30_000)
if result.acquired:
    print(result.lease.token)
else:
    print(result.current.owner)
```

Use `transaction()` when business writes and lease mutations must commit
or roll back together:

```python
with db.transaction() as tx:
    tx.execute("INSERT INTO jobs(payload) VALUES (?)", ["work"])
    claim = tx.claim("scheduler", "worker-a", ttl_ms=30_000)
    if not claim.acquired:
        tx.rollback()
```

The supported V1 transaction shape is the `with` block. Direct
`tx = db.transaction(); ...; tx.commit()` usage may work today, but it
is not the documented contract; use the context manager so rollback on
exceptions is deterministic.

While a transaction is active, use the `tx` object for all lease work.
Top-level `db.claim`, `db.renew`, `db.release`, and `db.inspect` calls
raise `BouncerError` until the transaction finishes.

`tx.execute(sql, params=None)` is a single-statement helper. It binds
positional parameters through SQLite and raises `BouncerError` for SQL
syntax errors or multi-statement strings.

Build and test from the repo root:

```bash
make build-py
make test-python
```
