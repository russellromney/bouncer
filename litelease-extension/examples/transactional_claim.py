from __future__ import annotations

from pathlib import Path
import sqlite3
import sys
import time


def extension_path() -> str:
    if sys.platform == "darwin":
        name = "liblitelease_ext.dylib"
    elif sys.platform == "win32":
        name = "litelease_ext.dll"
    else:
        name = "liblitelease_ext.so"
    return str(Path("target/release") / name)


def main() -> None:
    conn = sqlite3.connect("app.sqlite3")
    conn.enable_load_extension(True)
    conn.load_extension(extension_path())

    conn.execute("SELECT litelease_bootstrap()")
    conn.execute("CREATE TABLE IF NOT EXISTS jobs (payload TEXT NOT NULL)")
    conn.execute("BEGIN IMMEDIATE")
    conn.execute("INSERT INTO jobs(payload) VALUES (?)", ["work"])

    now_ms = int(time.time() * 1000)
    token = conn.execute(
        "SELECT litelease_claim(?, ?, ?, ?)",
        ("scheduler", "worker-a", 30_000, now_ms),
    ).fetchone()[0]

    conn.commit()
    print({"token": token})


if __name__ == "__main__":
    main()
