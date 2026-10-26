import 'just/common.just'
import 'just/server.just'
import 'just/mcp.just'

# List all commands
default:
    @just --list

# Run every check before pushing
[group('ci')]
ci:
