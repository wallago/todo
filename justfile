import 'just/common.just'

# List all commands
default:
    @just --list

# Run every check before pushing
[group('ci')]
ci:
