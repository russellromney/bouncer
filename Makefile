.PHONY: help test-rust build-ext build-py test-python test

help:
	@echo "bouncer development targets"
	@echo ""
	@echo "  make test-rust   - cargo test for Rust core and wrapper"
	@echo "  make build-ext   - build the SQLite loadable extension"
	@echo "  make build-py    - build/install the Python extension with maturin"
	@echo "  make test-python - build extension + Python package and run pytest"
	@echo "  make test        - rust + python"

test-rust:
	cargo test -p bouncer -p bouncer-core

build-ext:
	cargo build -p bouncer-extension

build-py:
	uv run --group dev maturin develop --manifest-path packages/bouncer-py/Cargo.toml

test-python: build-ext build-py
	uv run --group dev pytest packages/bouncer-py/tests

test: test-rust test-python
