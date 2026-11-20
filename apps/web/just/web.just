# ── Dev ───────────────────────────────────────────────────────

# Install the locked dependencies
[group('dev')]
install:
    pnpm install --frozen-lockfile

# Serve the app with hot reload
[group('dev')]
dev: install
    pnpm exec vite

# Build the site into `dist/`. Usage: just build [VITE_ARGS]
[group('dev')]
build *ARGS: install
    pnpm exec vite build {{ ARGS }}

# Type-check every source file (fast feedback, no build)
[group('dev')]
check: install
    pnpm exec tsc --noEmit
    pnpm exec tsc --noEmit -p tsconfig.sw.json

# Run the test suite. Usage: just test [FILTER]
[group('dev')]
test *ARGS: install
    pnpm exec vitest run {{ ARGS }}

# Format sources and sort imports in place
[group('dev')]
fmt: install
    pnpm exec biome check --write --linter-enabled=false

# ── Quality ───────────────────────────────────────────────────

# Check formatting and import order
[group('quality')]
fmt-check: install
    pnpm exec biome check --linter-enabled=false

# Lint, treating warnings as errors
[group('quality')]
lint: install
    pnpm exec biome lint --error-on-warnings
