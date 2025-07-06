.PHONY: all check test fmt fmt-check clippy clean

# Default target
all: fmt clippy test

# Check if code compiles
check:
	cargo check --all-features

# Run tests
test:
	cargo test

# Format code
fmt:
	cargo fmt --all

# Check formatting (CI mode)
fmt-check:
	cargo fmt --all -- --check

# Run clippy lints
clippy:
	cargo clippy -- -D warnings -D clippy::uninlined-format-args

# Clean build artifacts
clean:
	cargo clean

# Run all CI checks locally (matches GitHub Actions)
ci: fmt-check clippy test