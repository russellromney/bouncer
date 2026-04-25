# Bouncer Roadmap

## Summary

Bouncer should be the sharpest "who owns this right now?" wrapper on top of Honker.

Its job is not to become a scheduler or workflow system. Its job is to provide a durable lease with expiry and fencing in the same SQLite file the app already uses.

## Current status

This repo is a stub.

The intended model is:

- `honker`
  generic queue / wake / retry substrate
- `bouncer-honker`
  Bouncer-specific schema and SQLite contract
- `bouncer`
  thin language bindings

## First build steps

1. Define the resource / lease / fencing schema.
2. Write the SQLite contract in `bouncer-honker`.
3. Keep the initial binding tiny: open DB, claim, renew, release, inspect.
4. Add one binding first, then expand only if the contract feels stable.

## V1 nouns

- resource
- owner
- lease
- fencing token

## Success criteria

- one current owner per named resource
- expiry is durable and inspectable
- fencing token increments on successful claim
- bindings do not reimplement semantics
