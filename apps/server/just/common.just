# ── Dev ───────────────────────────────────────────────────────

# Run the app with `config.toml`. Usage: just run -- [ARGS]
[group('dev')]
run *ARGS:
    cargo run -- --config config.toml {{ ARGS }}

# Build the optimized release binary
[group('dev')]
build:
    cargo build --release

# Type-check the whole workspace (fast feedback, no binary)
[group('dev')]
check:
    cargo check --all-targets

# Run the test suite. Usage: just test [FILTER]
[group('dev')]
test *ARGS:
    cargo test {{ ARGS }}

# Format Rust and TOML sources in place
[group('dev')]
fmt:
    cargo fmt
    taplo fmt

# ── Quality ───────────────────────────────────────────────────

# Check Rust formatting
[group('quality')]
fmt-check:
    cargo fmt --check

# Check TOML formatting
[group('quality')]
toml-check:
    taplo fmt --check --diff

# Lint with clippy, treating warnings as errors
[group('quality')]
lint:
    cargo clippy --all-targets -- -D warnings

# Build the docs, treating warnings as errors
[group('quality')]
doc:
    RUSTDOCFLAGS=-Dwarnings cargo doc --no-deps
