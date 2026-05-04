.PHONY: help test-rust build-ext dist-ext smoke-ext test

help:
	@echo "litelease development targets"
	@echo ""
	@echo "  make test-rust   - cargo test for Rust core, Litelease wrapper, and extension artifact"
	@echo "  make build-ext   - build the SQLite loadable extension in release mode"
	@echo "  make dist-ext    - stage the current-platform release asset + sha256 in dist/"
	@echo "  make smoke-ext   - prove the release-built extension loads and runs"
	@echo "  make test        - alias for test-rust"

test-rust:
	cargo test -p litelease -p litelease-core

build-ext:
	cargo build -p litelease-extension --release

dist-ext: build-ext
	./scripts/stage_extension_dist.sh

smoke-ext:
	cargo test -p litelease --test extension_load release_extension_artifact_loads_full_smoke_path -- --exact

test: test-rust
