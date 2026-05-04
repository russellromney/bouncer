.PHONY: help test-rust build-ext test

help:
	@echo "bouncer development targets"
	@echo ""
	@echo "  make test-rust   - cargo test for Rust core, wrapper, and extension artifact"
	@echo "  make build-ext   - build the SQLite loadable extension"
	@echo "  make test        - alias for test-rust"

test-rust:
	cargo test -p bouncer -p bouncer-core

build-ext:
	cargo build -p bouncer-extension

test: test-rust
