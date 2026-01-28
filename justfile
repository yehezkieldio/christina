default:
    @just --list

check:
    cargo check

clippy:
    cargo clippy --all-targets --all-features -- -D warnings

test:
    cargo nextest run
test-one name:
    cargo nextest run {{name}}

format-check:
    cargo fmt --all -- --check

format:
    cargo fmt --all
fmt: format
