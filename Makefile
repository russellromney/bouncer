.PHONY: help test-rust build-ext dist-ext smoke-ext test

help:
	@echo "bouncer development targets"
	@echo ""
	@echo "  make test-rust   - cargo test for Rust core, wrapper, and extension artifact"
	@echo "  make build-ext   - build the SQLite loadable extension in release mode"
	@echo "  make dist-ext    - stage the current-platform release asset + sha256 in dist/"
	@echo "  make smoke-ext   - prove the release-built extension loads and runs"
	@echo "  make test        - alias for test-rust"

test-rust:
	cargo test -p bouncer -p bouncer-core

build-ext:
	cargo build -p bouncer-extension --release

dist-ext: build-ext
	./scripts/stage_extension_dist.sh

smoke-ext:
	cargo test -p bouncer --test extension_load release_extension_artifact_loads_full_smoke_path -- --exact

test: test-rust
