# ── Shared crates ─────────────────────────────────────────────

# The libraries in crates/, which no app's `ci` covers
libs := "-p todo-core -p todo-md -p todo-store -p todo-client"

# Check formatting, lint, test and document the shared crates
[group('quality')]
crates:
    cargo fmt --check {{ libs }}
    taplo fmt --check --diff Cargo.toml crates/*/Cargo.toml
    cargo clippy --all-targets {{ libs }} -- -D warnings
    cargo test {{ libs }}
    RUSTDOCFLAGS=-Dwarnings cargo doc --no-deps {{ libs }}
