# ── Nix ───────────────────────────────────────────────────────

# Lint Nix (statix) and report dead Nix code (deadnix)
[group('quality')]
nix-lint:
    statix check .
    deadnix --fail .

# Build & check every flake output on all systems
[group('dev')]
flake-check:
    nix flake check --print-build-logs --all-systems

# Update all flake inputs to their latest revisions
[group('dev')]
flake-update:
    nix flake update
