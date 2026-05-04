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
    now_ms = int(time.time() * 1000)
    token = conn.execute(
        "SELECT litelease_claim(?, ?, ?, ?)",
        ("scheduler", "worker-a", 30_000, now_ms),
    ).fetchone()[0]

    owner = conn.execute(
        "SELECT litelease_owner(?, ?)",
        ("scheduler", now_ms),
    ).fetchone()[0]

    print({"owner": owner, "token": token})


if __name__ == "__main__":
    main()
