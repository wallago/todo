# ── Release ───────────────────────────────────────────────────

# Check commit messages follow Conventional Commits. Usage: just commits [RANGE]
[group('release')]
commits RANGE="HEAD":
    committed -vv {{ RANGE }}

# Regenerate CHANGELOG.md from git history
[group('release')]
changelog:
    git-cliff -o CHANGELOG.md
