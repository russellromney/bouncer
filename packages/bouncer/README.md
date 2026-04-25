# bouncer package

Thin binding package for `bouncer-honker`.

The package should:

- open an existing SQLite database that already uses Honker
- call the Bouncer SQLite contract
- expose a tiny ergonomic API

The package should not:

- reimplement lease semantics
- invent a second state machine
- hide the underlying model too much
