default:
    @just --list

# Check the project for errors without building
check:
    cargo check

# Lint the project using Clippy
clippy:
    cargo clippy --all-targets --all-features -- -D warnings

# Run the test suite using Nextest
test:
    cargo nextest run

# Check code formatting without making changes
format-check:
    cargo fmt --all -- --check

# Format the codebase
format:
    cargo fmt --all
fmt: format
