from __future__ import annotations

import json
import sqlite3
import subprocess
import sys
import time
from pathlib import Path

import pytest

import bouncer


def extension_path() -> Path:
    root = Path(__file__).resolve().parents[3]
    target = root / "target" / "debug"
    if sys.platform == "darwin":
        artifact = target / "libbouncer_ext.dylib"
    elif sys.platform.startswith("win"):
        artifact = target / "bouncer_ext.dll"
    else:
        artifact = target / "libbouncer_ext.so"

    if not artifact.exists():
        pytest.fail(f"missing bouncer extension artifact: {artifact}; run `make build-ext`")
    return artifact


def connect_sql(path: Path) -> sqlite3.Connection:
    conn = sqlite3.connect(path)
    conn.enable_load_extension(True)
    conn.load_extension(str(extension_path()))
    return conn


def create_business_table(path: Path) -> None:
    with sqlite3.connect(path) as conn:
        conn.execute("CREATE TABLE business_events (note TEXT NOT NULL)")


def business_count(path: Path) -> int:
    with sqlite3.connect(path) as conn:
        row = conn.execute("SELECT COUNT(*) FROM business_events").fetchone()
    return int(row[0])


def test_explicit_bootstrap_is_required(tmp_path: Path) -> None:
    db = bouncer.open(tmp_path / "app.sqlite3")

    with pytest.raises(bouncer.BouncerError, match="bouncer_resources|no such table"):
        db.claim("scheduler", "worker-a", ttl_ms=30_000)


def test_full_lifecycle(tmp_path: Path) -> None:
    db = bouncer.open(tmp_path / "app.sqlite3")
    db.bootstrap()

    claim = db.claim("scheduler", "worker-a", ttl_ms=30_000)
    assert claim.acquired
    assert claim.lease is not None
    assert claim.lease.token == 1

    busy = db.claim("scheduler", "worker-b", ttl_ms=30_000)
    assert not busy.acquired
    assert busy.current is not None
    assert busy.current.owner == "worker-a"

    inspected = db.inspect("scheduler")
    assert inspected == claim.lease

    renewed = db.renew("scheduler", "worker-a", ttl_ms=60_000)
    assert renewed.renewed
    assert renewed.lease is not None
    assert renewed.lease.token == claim.lease.token
    assert renewed.lease.lease_expires_at_ms >= claim.lease.lease_expires_at_ms

    rejected_renew = db.renew("scheduler", "worker-b", ttl_ms=60_000)
    assert not rejected_renew.renewed
    assert rejected_renew.current is not None
    assert rejected_renew.current.owner == "worker-a"

    released = db.release("scheduler", "worker-a")
    assert released.released
    assert released.name == "scheduler"
    assert released.token == claim.lease.token
    assert db.inspect("scheduler") is None


def test_python_claim_is_visible_to_sql_extension(tmp_path: Path) -> None:
    path = tmp_path / "app.sqlite3"
    db = bouncer.open(path)
    db.bootstrap()

    claim = db.claim("scheduler", "worker-a", ttl_ms=30_000)
    assert claim.lease is not None

    conn = connect_sql(path)
    try:
        now_ms = claim.lease.lease_expires_at_ms - 1
        owner = conn.execute(
            "SELECT bouncer_owner(?, ?)", ("scheduler", now_ms)
        ).fetchone()[0]
        token = conn.execute("SELECT bouncer_token(?)", ("scheduler",)).fetchone()[0]
    finally:
        conn.close()

    assert owner == "worker-a"
    assert token == claim.lease.token


def test_sql_created_lease_is_visible_to_python(tmp_path: Path) -> None:
    path = tmp_path / "app.sqlite3"
    conn = connect_sql(path)
    try:
        conn.execute("SELECT bouncer_bootstrap()")
        now_ms = int(time.time() * 1000)
        token = conn.execute(
            "SELECT bouncer_claim(?, ?, ?, ?)",
            ("scheduler", "sql-worker", 60_000, now_ms),
        ).fetchone()[0]
        conn.commit()
    finally:
        conn.close()

    db = bouncer.open(path)
    lease = db.inspect("scheduler")

    assert lease is not None
    assert lease.owner == "sql-worker"
    assert lease.token == token


def test_transaction_commit_persists_business_write_and_lease(tmp_path: Path) -> None:
    path = tmp_path / "app.sqlite3"
    create_business_table(path)
    db = bouncer.open(path)
    db.bootstrap()

    with db.transaction() as tx:
        affected = tx.execute("INSERT INTO business_events(note) VALUES (?)", ["commit"])
        claim = tx.claim("scheduler", "worker-a", ttl_ms=30_000)

    assert affected == 1
    assert claim.acquired
    assert business_count(path) == 1
    assert db.inspect("scheduler") is not None


def test_transaction_inspect_returns_live_lease(tmp_path: Path) -> None:
    db = bouncer.open(tmp_path / "app.sqlite3")
    db.bootstrap()

    with db.transaction() as tx:
        claim = tx.claim("scheduler", "worker-a", ttl_ms=30_000)
        inspected = tx.inspect("scheduler")

    assert claim.lease is not None
    assert inspected == claim.lease


def test_transaction_renew_extends_lease(tmp_path: Path) -> None:
    db = bouncer.open(tmp_path / "app.sqlite3")
    db.bootstrap()

    with db.transaction() as tx:
        claim = tx.claim("scheduler", "worker-a", ttl_ms=30_000)
        renewed = tx.renew("scheduler", "worker-a", ttl_ms=60_000)

    assert claim.lease is not None
    assert renewed.renewed
    assert renewed.lease is not None
    assert renewed.lease.token == claim.lease.token
    assert renewed.lease.lease_expires_at_ms >= claim.lease.lease_expires_at_ms


def test_transaction_release_clears_owner(tmp_path: Path) -> None:
    db = bouncer.open(tmp_path / "app.sqlite3")
    db.bootstrap()

    with db.transaction() as tx:
        claim = tx.claim("scheduler", "worker-a", ttl_ms=30_000)
        released = tx.release("scheduler", "worker-a")
        inspected = tx.inspect("scheduler")

    assert claim.lease is not None
    assert released.released
    assert released.token == claim.lease.token
    assert inspected is None
    assert db.inspect("scheduler") is None


def test_transaction_rollback_discards_business_write_and_lease(tmp_path: Path) -> None:
    path = tmp_path / "app.sqlite3"
    create_business_table(path)
    db = bouncer.open(path)
    db.bootstrap()

    with pytest.raises(RuntimeError, match="boom"):
        with db.transaction() as tx:
            tx.execute("INSERT INTO business_events(note) VALUES (?)", ["rollback"])
            tx.claim("scheduler", "worker-a", ttl_ms=30_000)
            raise RuntimeError("boom")

    assert business_count(path) == 0
    assert db.inspect("scheduler") is None


def test_context_manager_explicit_finish_is_terminal_and_exit_is_noop(tmp_path: Path) -> None:
    path = tmp_path / "app.sqlite3"
    create_business_table(path)
    db = bouncer.open(path)
    db.bootstrap()

    with db.transaction() as tx:
        tx.execute("INSERT INTO business_events(note) VALUES (?)", ["done"])
        tx.commit()
        with pytest.raises(bouncer.BouncerError, match="already finished"):
            tx.inspect("scheduler")

    assert business_count(path) == 1

    with db.transaction() as tx:
        tx.execute("INSERT INTO business_events(note) VALUES (?)", ["gone"])
        tx.rollback()
        with pytest.raises(bouncer.BouncerError, match="already finished"):
            tx.execute("INSERT INTO business_events(note) VALUES (?)", ["also-gone"])

    assert business_count(path) == 1


def test_top_level_operations_raise_during_active_transaction(tmp_path: Path) -> None:
    db = bouncer.open(tmp_path / "app.sqlite3")
    db.bootstrap()

    with db.transaction() as tx:
        with pytest.raises(bouncer.BouncerError, match="transaction is active"):
            db.inspect("scheduler")
        with pytest.raises(bouncer.BouncerError, match="transaction is active"):
            db.claim("scheduler", "worker-a", ttl_ms=30_000)
        with pytest.raises(bouncer.BouncerError, match="transaction is active"):
            db.renew("scheduler", "worker-a", ttl_ms=30_000)
        with pytest.raises(bouncer.BouncerError, match="transaction is active"):
            db.release("scheduler", "worker-a")

        claim = tx.claim("scheduler", "worker-a", ttl_ms=30_000)
        assert claim.acquired


def test_overlapping_transactions_fail_loudly(tmp_path: Path) -> None:
    db = bouncer.open(tmp_path / "app.sqlite3")
    db.bootstrap()

    with db.transaction():
        with pytest.raises(bouncer.BouncerError, match="transaction is active"):
            with db.transaction():
                pass


def test_errors_map_to_bouncer_error(tmp_path: Path) -> None:
    db = bouncer.open(tmp_path / "app.sqlite3")
    db.bootstrap()

    with pytest.raises(bouncer.BouncerError, match="ttl_ms must be positive"):
        db.claim("scheduler", "worker-a", ttl_ms=0)


def test_transaction_execute_sql_errors_map_to_bouncer_error(tmp_path: Path) -> None:
    db = bouncer.open(tmp_path / "app.sqlite3")
    db.bootstrap()

    with db.transaction() as tx:
        with pytest.raises(bouncer.BouncerError, match="syntax error|near"):
            tx.execute("SELLECT 1")


def test_transaction_execute_rejects_multiple_statements(tmp_path: Path) -> None:
    path = tmp_path / "app.sqlite3"
    create_business_table(path)
    db = bouncer.open(path)
    db.bootstrap()

    with db.transaction() as tx:
        with pytest.raises(bouncer.BouncerError, match="Multiple statements"):
            tx.execute(
                "INSERT INTO business_events(note) VALUES (?); "
                "INSERT INTO business_events(note) VALUES (?)",
                ["one", "two"],
            )

    assert business_count(path) == 0


def test_transaction_without_enter_raises(tmp_path: Path) -> None:
    db = bouncer.open(tmp_path / "app.sqlite3")
    db.bootstrap()

    tx = db.transaction()

    with pytest.raises(bouncer.BouncerError, match="not been entered"):
        tx.claim("scheduler", "worker-a", ttl_ms=30_000)
    with pytest.raises(bouncer.BouncerError, match="not been entered"):
        tx.inspect("scheduler")
    with pytest.raises(bouncer.BouncerError, match="not been entered"):
        tx.commit()


def test_transaction_without_enter_does_not_lock_database(tmp_path: Path) -> None:
    path = tmp_path / "app.sqlite3"
    db = bouncer.open(path)
    db.bootstrap()

    tx = db.transaction()

    other = sqlite3.connect(path, timeout=0.5)
    try:
        other.execute("CREATE TABLE other_writes (id INTEGER PRIMARY KEY)")
        other.commit()
    finally:
        other.close()

    with tx:
        claim = tx.claim("scheduler", "worker-a", ttl_ms=30_000)
    assert claim.acquired


def test_transaction_begin_failure_can_reenter_same_instance(tmp_path: Path) -> None:
    path = tmp_path / "app.sqlite3"
    db = bouncer.open(path)
    db.bootstrap()

    blocker = sqlite3.connect(path, timeout=0.5)
    blocker.execute("BEGIN IMMEDIATE")
    try:
        tx = db.transaction()
        with pytest.raises(bouncer.BouncerError, match="locked|busy"):
            tx.__enter__()
    finally:
        blocker.rollback()
        blocker.close()

    with tx:
        claim = tx.claim("scheduler", "worker-a", ttl_ms=30_000)

    assert claim.acquired


def test_transaction_is_single_use(tmp_path: Path) -> None:
    db = bouncer.open(tmp_path / "app.sqlite3")
    db.bootstrap()

    tx = db.transaction()
    with tx:
        pass

    with pytest.raises(bouncer.BouncerError, match="already finished|already entered"):
        with tx:
            pass


def test_transaction_execute_binds_positional_parameters(tmp_path: Path) -> None:
    path = tmp_path / "app.sqlite3"
    create_business_table(path)
    db = bouncer.open(path)
    db.bootstrap()
    payload = "x'); DROP TABLE business_events; --"

    with db.transaction() as tx:
        affected = tx.execute("INSERT INTO business_events(note) VALUES (?)", [payload])

    assert affected == 1
    with sqlite3.connect(path) as conn:
        row = conn.execute("SELECT note FROM business_events").fetchone()
    assert row[0] == payload


def test_python_cross_surface_interop(tmp_path: Path) -> None:
    path = tmp_path / "app.sqlite3"
    db = bouncer.open(path)
    db.bootstrap()

    # Python claims with a long TTL so the lease stays live.
    claim = db.claim("scheduler", "python-worker", ttl_ms=86_400_000)
    assert claim.acquired
    assert claim.lease is not None
    first_token = claim.lease.token

    # SQL extension sees the same lease.
    conn = connect_sql(path)
    try:
        owner = conn.execute(
            "SELECT bouncer_owner(?, ?)", ("scheduler", claim.lease.lease_expires_at_ms - 1)
        ).fetchone()[0]
        token = conn.execute("SELECT bouncer_token(?)", ("scheduler",)).fetchone()[0]
    finally:
        conn.close()

    assert owner == "python-worker"
    assert token == first_token

    # SQL extension releases.
    conn = connect_sql(path)
    try:
        released = conn.execute(
            "SELECT bouncer_release(?, ?, ?)",
            ("scheduler", "python-worker", claim.lease.lease_expires_at_ms - 1),
        ).fetchone()[0]
    finally:
        conn.close()

    assert released == 1

    # Python sees no owner after SQL release.
    assert db.inspect("scheduler") is None

    # Python reclaims; token should increase.
    reclaim = db.claim("scheduler", "next-worker", ttl_ms=86_400_000)
    assert reclaim.acquired
    assert reclaim.lease is not None
    assert reclaim.lease.token > first_token

    # SQL sees the new owner and token.
    conn = connect_sql(path)
    try:
        sql_owner_val = conn.execute(
            "SELECT bouncer_owner(?, ?)",
            ("scheduler", reclaim.lease.lease_expires_at_ms - 1),
        ).fetchone()[0]
        sql_token_val = conn.execute(
            "SELECT bouncer_token(?)", ("scheduler",)
        ).fetchone()[0]
    finally:
        conn.close()

    assert sql_owner_val == "next-worker"
    assert sql_token_val == reclaim.lease.token


def test_python_bootstrap_fails_on_drifted_schema(tmp_path: Path) -> None:
    path = tmp_path / "app.sqlite3"
    with sqlite3.connect(path) as conn:
        conn.execute(
            "CREATE TABLE bouncer_resources ("
            "name TEXT PRIMARY KEY, owner TEXT, token INTEGER NOT NULL, "
            "lease_expires_at_ms INTEGER, created_at_ms INTEGER NOT NULL)"
        )

    db = bouncer.open(path)
    with pytest.raises(bouncer.BouncerError, match="schema mismatch|SchemaMismatch"):
        db.bootstrap()


def test_three_surfaces_observe_same_state(tmp_path: Path) -> None:
    repo_root = Path(__file__).resolve().parents[3]
    cargo = "cargo"

    path = tmp_path / "three_surface.sqlite3"
    db = bouncer.open(path)
    db.bootstrap()

    claim = db.claim("scheduler", "python-owner", ttl_ms=86_400_000)
    assert claim.acquired
    assert claim.lease is not None
    first_token = claim.lease.token

    conn = connect_sql(path)
    try:
        sql_owner = conn.execute(
            "SELECT bouncer_owner(?, ?)",
            ("scheduler", claim.lease.lease_expires_at_ms - 1),
        ).fetchone()[0]
        sql_token = conn.execute("SELECT bouncer_token(?)", ("scheduler",)).fetchone()[0]
    finally:
        conn.close()
    assert sql_owner == "python-owner"
    assert sql_token == first_token

    result = subprocess.run(
        [cargo, "run", "--example", "three_surface_observer", "--", str(path), "scheduler"],
        capture_output=True,
        text=True,
        check=True,
        cwd=str(repo_root),
    )
    wrapper_view = json.loads(result.stdout.strip())
    assert wrapper_view["exists"] is True
    assert wrapper_view["owner"] == "python-owner"
    assert wrapper_view["token"] == first_token

    conn = connect_sql(path)
    try:
        released = conn.execute(
            "SELECT bouncer_release(?, ?, ?)",
            ("scheduler", "python-owner", claim.lease.lease_expires_at_ms - 1),
        ).fetchone()[0]
    finally:
        conn.close()
    assert released == 1

    assert db.inspect("scheduler") is None

    result = subprocess.run(
        [cargo, "run", "--example", "three_surface_observer", "--", str(path), "scheduler"],
        capture_output=True,
        text=True,
        check=True,
        cwd=str(repo_root),
    )
    wrapper_view = json.loads(result.stdout.strip())
    assert wrapper_view["exists"] is False

    reclaim = db.claim("scheduler", "next-owner", ttl_ms=86_400_000)
    assert reclaim.acquired
    assert reclaim.lease is not None
    assert reclaim.lease.token > first_token

    conn = connect_sql(path)
    try:
        sql_owner2 = conn.execute(
            "SELECT bouncer_owner(?, ?)",
            ("scheduler", reclaim.lease.lease_expires_at_ms - 1),
        ).fetchone()[0]
        sql_token2 = conn.execute("SELECT bouncer_token(?)", ("scheduler",)).fetchone()[0]
    finally:
        conn.close()
    assert sql_owner2 == "next-owner"
    assert sql_token2 == reclaim.lease.token

    result = subprocess.run(
        [cargo, "run", "--example", "three_surface_observer", "--", str(path), "scheduler"],
        capture_output=True,
        text=True,
        check=True,
        cwd=str(repo_root),
    )
    wrapper_view = json.loads(result.stdout.strip())
    assert wrapper_view["exists"] is True
    assert wrapper_view["owner"] == "next-owner"
    assert wrapper_view["token"] == reclaim.lease.token
