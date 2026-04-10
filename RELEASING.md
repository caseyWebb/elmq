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
5. **npm** — platform-specific packages and the root `@caseywebb/elmq` package are published to npmjs.org
6. **Homebrew** — the Homebrew formula in [caseyWebb/homebrew-tap](https://github.com/caseyWebb/homebrew-tap) is automatically updated

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

One repository secret is needed at https://github.com/caseyWebb/elmq/settings/secrets/actions:

### `RELEASE_TOKEN`

A single fine-grained GitHub personal access token used by release-please (to create PRs/releases that trigger downstream workflows) and the release workflow (to push Homebrew formula updates).

1. Go to [GitHub Settings > Developer settings > Fine-grained tokens](https://github.com/settings/tokens?type=beta)
2. Create a token with repository access to **both** `caseyWebb/elmq` and `caseyWebb/homebrew-tap`
3. Grant permissions: **Contents: Read and write**, **Pull requests: Read and write**
4. Add it as a repository secret named `RELEASE_TOKEN` at https://github.com/caseyWebb/elmq/settings/secrets/actions

### npm Trusted Publishing

npm packages are published via [Trusted Publishing](https://docs.npmjs.com/generating-provenance-statements) using GitHub Actions OIDC — no npm token secret is needed. Each `@caseywebb/elmq*` package must have Trusted Publishing configured on npmjs.org:

1. Go to the package settings page on npmjs.com (e.g. `https://www.npmjs.com/package/@caseywebb/elmq/access`)
2. Under "Publishing access", configure GitHub Actions as a trusted publisher
3. Set repository: `caseyWebb/elmq`, workflow: `release.yml`
4. Repeat for each platform package (`elmq-darwin-arm64`, `elmq-darwin-x64`, `elmq-linux-arm64`, `elmq-linux-x64`)

## Branch Protection

The `main` branch has a ruleset requiring all CI checks to pass before merging:

- Format check
- Clippy lint
- Tests (Ubuntu, macOS, Windows)
- PR title follows conventional commit format

Only squash merges are allowed.

Repository admins can bypass these rules when needed.
