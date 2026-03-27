---
update-when: release process, CI workflows, or versioning strategy changes
---

# Releasing

elmq uses [release-please](https://github.com/googleapis/release-please) for automated releases with [SemVer](https://semver.org/) versioning.

## How It Works

1. **Squash merge to main** — PRs are squash-merged using the PR title as the commit message. PR titles must follow conventional commit format (enforced by CI).
2. **Release PR** — release-please creates (or updates) a PR that bumps the version in `Cargo.toml` and updates `CHANGELOG.md`
3. **Publish** — merging the release PR creates a GitHub Release with a git tag (`vX.Y.Z`)
4. **Build** — the release workflow builds binaries for 5 targets and attaches them to the release
5. **Homebrew** — the Homebrew formula in [caseyWebb/homebrew-tap](https://github.com/caseyWebb/homebrew-tap) is automatically updated

## Version Bumps

Version bumps are determined by conventional commit prefixes:

| Commit | Bump | Example |
|--------|------|---------|
| `fix:` | patch | `0.1.0` → `0.1.1` |
| `feat:` | minor | `0.1.0` → `0.2.0` |
| `feat!:` or `BREAKING CHANGE` | minor (pre-1.0) | `0.1.0` → `0.2.0` |
| `feat!:` or `BREAKING CHANGE` | major (post-1.0) | `1.0.0` → `2.0.0` |

## Binary Targets

| Target | OS | Arch |
|--------|----|------|
| `x86_64-unknown-linux-musl` | Linux | x86_64 |
| `aarch64-unknown-linux-musl` | Linux | ARM64 |
| `x86_64-apple-darwin` | macOS | Intel |
| `aarch64-apple-darwin` | macOS | Apple Silicon |
| `x86_64-pc-windows-msvc` | Windows | x86_64 |

Linux binaries are statically linked (musl) for maximum portability.

## Required Secrets

The release workflow requires a `HOMEBREW_TAP_TOKEN` repository secret — a fine-grained GitHub personal access token with **Contents: Read and write** permission scoped to the `caseyWebb/homebrew-tap` repository.

To create:

1. Go to [GitHub Settings > Developer settings > Fine-grained tokens](https://github.com/settings/tokens?type=beta)
2. Create a token with repository access to `caseyWebb/homebrew-tap` only
3. Grant **Contents: Read and write** permission
4. Add it as a repository secret named `HOMEBREW_TAP_TOKEN` at https://github.com/caseyWebb/elmq/settings/secrets/actions

## Branch Protection

The `main` branch has a ruleset requiring all CI checks to pass before merging:

- Format check
- Clippy lint
- Tests (Ubuntu, macOS, Windows)
- PR title follows conventional commit format

Only squash merges are allowed.

Repository admins can bypass these rules when needed.
