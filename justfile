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

# Format the code and check for formatting issues
format:
    cargo fmt --all -- --check
