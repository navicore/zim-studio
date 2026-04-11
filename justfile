# zim-studio — justfile
#
# This is the SOURCE OF TRUTH for build/test/lint operations.
# Both local development and GitHub Actions CI call these recipes —
# there is no duplication of build logic in workflow YAML.

# Run all CI checks (same as GitHub Actions!)
# This is what developers should run before pushing.
ci: fmt-check lint test build
    @echo "Safe to push to GitHub - CI will pass."

# Format code
fmt:
    cargo fmt --all

# Check formatting (CI mode)
fmt-check:
    cargo fmt --all -- --check

# Run clippy lints — warnings are errors
lint:
    cargo clippy --locked --workspace --all-targets --all-features -- -D warnings -D clippy::all -D clippy::uninlined-format-args

# Run the test suite
test:
    cargo test --locked --workspace --all-targets --all-features

# Build release binary
build:
    cargo build --locked --release --all-features

# Clean build artifacts
clean:
    cargo clean
