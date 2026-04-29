# Plan

## Goal

Rename the core crate before Python spreads the wrong name.

`bouncer-honker` was family-flavored naming, not architecture. If
Honker eventually depends on Bouncer for scheduler ownership or other
single-machine coordination, the dependency direction should read:

```text
honker -> bouncer-core
```

not:

```text
honker -> bouncer-honker
```

That second sentence is goose noise.

## Phase outcome

At the end of Phase 008:

- the core crate is named `bouncer-core`
- the core directory is `bouncer-core/`
- Rust imports use `bouncer_core`
- current docs use `bouncer-core`
- Phase 009 Python planning uses `bouncer-core`
- historical phase text may still mention `bouncer-honker` when that
  was the name at the time

## Implementation steps

1. Rename the directory with git so history follows it.
2. Update Cargo workspace members and dependency declarations.
3. Update Rust imports in the wrapper and extension.
4. Update current docs and active future plans.
5. Add a changelog entry explaining the rename.
6. Run Rust checks.

## Files likely to change

- `Cargo.toml`
- `Cargo.lock`
- `bouncer-core/**`
- `bouncer-extension/**`
- `packages/bouncer/**`
- `README.md`
- `ROADMAP.md`
- `SYSTEM.md`
- `CHANGELOG.md`
- `.intent/phases/008-core-crate-rename/*`
- `.intent/phases/009-python-binding/*`

## Areas that should not be touched

- lease semantics
- table names
- SQL function names
- Rust wrapper method names
- old phase artifacts except for the active Python plan renumbering

## Risks and assumptions

- Cargo package rename changes the Rust import path from
  `bouncer_honker` to `bouncer_core`.
- The old name may remain in historical append-only records. That is
  acceptable as long as current docs and active plans use the new name.
