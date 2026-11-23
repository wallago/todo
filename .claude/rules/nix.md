# Nix

The dev shell is the environment. Tooling comes from the flake: don't assume
anything is on the bare system, and use `nix run nixpkgs#<tool>` for one-offs.
A tool the repo needs goes in the flake's `ciTools` or `devTools`.

`.envrc` is `use flake`, so direnv loads the shell on `cd`. `.direnv/` is
gitignored and holds nixpkgs sources: exclude it from any repo-wide file scan.

Nix sources get `statix check` and `deadnix`. `nix flake check` is the real
verification: a change that only `nix build`s isn't checked.

`flake.lock` changes as a side effect of a dependency change, in its own
commit, never bundled into a feature diff.
