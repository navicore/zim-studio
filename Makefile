.PHONY: all check test fmt fmt-check clippy clean

# Default target
all: fmt clippy test

# Check if code compiles
check:
	cargo check --all-features

# Run tests
test:
	cargo test --all-features

# Format code
fmt:
	cargo fmt --all

# Check formatting (CI mode)
fmt-check:
	cargo fmt --all -- --check

# Run clippy lints
clippy:
	cargo clippy --all-features -- -D warnings -D clippy::uninlined-format-args

# Clean build artifacts
clean:
	cargo clean

# build artifacts
build:
	cargo build --features player

# build artifacts
install:
	cargo install --path . --features player

# Run all CI checks locally (matches GitHub Actions)
ci: fmt-check clippy test
