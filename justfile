import 'just/nix.just'
import 'just/udeps.just'
import 'just/deny.just'
import 'just/audit.just'
import 'just/editorconfig.just'
import 'just/typos.just'
import 'just/lychee.just'
import 'just/git.just'
import 'just/github.just'
import 'just/crates.just'
mod server 'apps/server'
mod tui 'apps/tui'

# List all commands
default:
    @just --list

# Run every check before pushing
[group('ci')]
ci:
    @just nix-lint
    @just udeps
    @just deny
    @just audit
    @just editorconfig
    @just typos
    @just links
    @just workflows
    @just crates
    @just server::ci
    @just tui::ci
