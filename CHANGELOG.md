# Changelog

## 0.9.0 (2026-05-07)

## What's Changed
* revert(plugin): drop self-hosted marketplace, use caseyWebb/claude-plugins by @caseyWebb in https://github.com/caseyWebb/elmq/pull/84
* chore(deps): Bump actions/setup-node from 4 to 6 by @dependabot[bot] in https://github.com/caseyWebb/elmq/pull/83
* chore(deps): Bump actions/upload-artifact from 4 to 7 by @dependabot[bot] in https://github.com/caseyWebb/elmq/pull/80
* chore(deps): Bump actions/download-artifact from 4 to 8 by @dependabot[bot] in https://github.com/caseyWebb/elmq/pull/81
* chore(deps): Bump actions/attest-build-provenance from 2 to 4 by @dependabot[bot] in https://github.com/caseyWebb/elmq/pull/82
* chore(deps): Bump clap from 4.6.0 to 4.6.1 by @dependabot[bot] in https://github.com/caseyWebb/elmq/pull/87
* chore(deps): Bump googleapis/release-please-action from 4 to 5 by @dependabot[bot] in https://github.com/caseyWebb/elmq/pull/86


**Full Changelog**: https://github.com/caseyWebb/elmq/compare/v0.8.0...v0.9.0

## 0.8.0 (2026-04-14)

## What's Changed
* fix: include README and LICENSE in release artifacts by @caseyWebb in https://github.com/caseyWebb/elmq/pull/74
* feat(refs): constructor-aware refs dispatch + variant rm advisory classifier by @caseyWebb in https://github.com/caseyWebb/elmq/pull/76
* feat!: reject invalid Elm edits via tree-sitter gates by @caseyWebb in https://github.com/caseyWebb/elmq/pull/77
* docs: add HYPOTHESIS.md stating the elmq experiment by @caseyWebb in https://github.com/caseyWebb/elmq/pull/78
* feat!: restructure edit surface to verb-first <scope> by @caseyWebb in https://github.com/caseyWebb/elmq/pull/79


**Full Changelog**: https://github.com/caseyWebb/elmq/compare/v0.7.4...v0.8.0

## 0.7.4 (2026-04-13)

## What's Changed
* fix: concurrency, release-only builds, npm trusted publishing by @caseyWebb in https://github.com/caseyWebb/elmq/pull/71
* fix: bump node to 24 for trusted publishing and benchmark consistency by @caseyWebb in https://github.com/caseyWebb/elmq/pull/73


**Full Changelog**: https://github.com/caseyWebb/elmq/compare/v0.7.3...v0.7.4

## 0.7.3 (2026-04-13)

## What's Changed
* fix: remove orphaned elm-spa-example submodule gitlink by @caseyWebb in https://github.com/caseyWebb/elmq/pull/68


**Full Changelog**: https://github.com/caseyWebb/elmq/compare/v0.7.2...v0.7.3

## 0.7.2 (2026-04-13)

## What's Changed
* fix: build before release to sidestep immutable release constraint by @caseyWebb in https://github.com/caseyWebb/elmq/pull/64
* fix: disable component prefix in release-please tags by @caseyWebb in https://github.com/caseyWebb/elmq/pull/65
* fix: use draft releases for immutable release compatibility by @caseyWebb in https://github.com/caseyWebb/elmq/pull/66


**Full Changelog**: https://github.com/caseyWebb/elmq/compare/v0.7.1...v0.7.2

## 0.7.1 (2026-04-10)

## What's Changed
* fix: build before release to avoid immutable release issues by @caseyWebb in https://github.com/caseyWebb/elmq/pull/62


**Full Changelog**: https://github.com/caseyWebb/elmq/compare/elmq-v0.7.0...elmq-v0.7.1

## 0.7.0 (2026-04-10)

## What's Changed
* feat: embed agent guide in binary, simplify release pipeline by @caseyWebb in https://github.com/caseyWebb/elmq/pull/59
* fix: split release into two workflows for re-runnability by @caseyWebb in https://github.com/caseyWebb/elmq/pull/61


**Full Changelog**: https://github.com/caseyWebb/elmq/compare/elmq-v0.6.0...elmq-v0.7.0

## 0.6.0 (2026-04-10)

## What's Changed
* fix: use draft releases to support immutable release assets by @caseyWebb in https://github.com/caseyWebb/elmq/pull/33
* feat: add SLSA build provenance attestations for release binaries by @caseyWebb in https://github.com/caseyWebb/elmq/pull/35
* chore(main): release 0.6.0 by @caseyWebb in https://github.com/caseyWebb/elmq/pull/34
* feat: add Claude Code plugin with SessionStart hook and release-please versioning by @caseyWebb in https://github.com/caseyWebb/elmq/pull/36
* fix: reset version to 0.5.0 after orphaned release-please bump by @caseyWebb in https://github.com/caseyWebb/elmq/pull/37
* chore(main): release 0.6.0 by @caseyWebb in https://github.com/caseyWebb/elmq/pull/38
* chore: reset release-please manifest to v0.5.0 by @caseyWebb in https://github.com/caseyWebb/elmq/pull/39
* feat: add release-please multi-package for independent plugin versioning by @caseyWebb in https://github.com/caseyWebb/elmq/pull/41
* chore: release main by @caseyWebb in https://github.com/caseyWebb/elmq/pull/42
* chore: reset release-please manifest to v0.5.0 by @caseyWebb in https://github.com/caseyWebb/elmq/pull/43
* chore: release main by @caseyWebb in https://github.com/caseyWebb/elmq/pull/44
* fix: add force-tag-creation for draft releases by @caseyWebb in https://github.com/caseyWebb/elmq/pull/45
* chore: release main by @caseyWebb in https://github.com/caseyWebb/elmq/pull/46
* fix: unify release-please and release into single workflow by @caseyWebb in https://github.com/caseyWebb/elmq/pull/47
* chore: reset version to 0.5.0 by @caseyWebb in https://github.com/caseyWebb/elmq/pull/49
* chore: release main by @caseyWebb in https://github.com/caseyWebb/elmq/pull/50
* chore: reset version to 0.5.0 by @caseyWebb in https://github.com/caseyWebb/elmq/pull/51
* chore: release main by @caseyWebb in https://github.com/caseyWebb/elmq/pull/52
* fix: use separate release PRs and reset version to 0.5.0 by @caseyWebb in https://github.com/caseyWebb/elmq/pull/53
* chore(main): release 0.6.0 by @caseyWebb in https://github.com/caseyWebb/elmq/pull/54
* fix: drop draft releases, revert to grouped PRs, reset to 0.5.0 by @caseyWebb in https://github.com/caseyWebb/elmq/pull/55
* chore: release main by @caseyWebb in https://github.com/caseyWebb/elmq/pull/56
* fix: include-component-in-tag and reset to 0.5.0 by @caseyWebb in https://github.com/caseyWebb/elmq/pull/57


**Full Changelog**: https://github.com/caseyWebb/elmq/compare/elmq-v0.5.0...elmq-v0.6.0

## 0.6.0 (2026-04-10)

## What's Changed
* fix: use draft releases to support immutable release assets by @caseyWebb in https://github.com/caseyWebb/elmq/pull/33
* feat: add SLSA build provenance attestations for release binaries by @caseyWebb in https://github.com/caseyWebb/elmq/pull/35
* chore(main): release 0.6.0 by @caseyWebb in https://github.com/caseyWebb/elmq/pull/34
* feat: add Claude Code plugin with SessionStart hook and release-please versioning by @caseyWebb in https://github.com/caseyWebb/elmq/pull/36
* fix: reset version to 0.5.0 after orphaned release-please bump by @caseyWebb in https://github.com/caseyWebb/elmq/pull/37
* chore(main): release 0.6.0 by @caseyWebb in https://github.com/caseyWebb/elmq/pull/38
* chore: reset release-please manifest to v0.5.0 by @caseyWebb in https://github.com/caseyWebb/elmq/pull/39
* feat: add release-please multi-package for independent plugin versioning by @caseyWebb in https://github.com/caseyWebb/elmq/pull/41
* chore: release main by @caseyWebb in https://github.com/caseyWebb/elmq/pull/42
* chore: reset release-please manifest to v0.5.0 by @caseyWebb in https://github.com/caseyWebb/elmq/pull/43
* chore: release main by @caseyWebb in https://github.com/caseyWebb/elmq/pull/44
* fix: add force-tag-creation for draft releases by @caseyWebb in https://github.com/caseyWebb/elmq/pull/45
* chore: release main by @caseyWebb in https://github.com/caseyWebb/elmq/pull/46
* fix: unify release-please and release into single workflow by @caseyWebb in https://github.com/caseyWebb/elmq/pull/47
* chore: reset version to 0.5.0 by @caseyWebb in https://github.com/caseyWebb/elmq/pull/49
* chore: release main by @caseyWebb in https://github.com/caseyWebb/elmq/pull/50
* chore: reset version to 0.5.0 by @caseyWebb in https://github.com/caseyWebb/elmq/pull/51
* chore: release main by @caseyWebb in https://github.com/caseyWebb/elmq/pull/52
* fix: use separate release PRs and reset version to 0.5.0 by @caseyWebb in https://github.com/caseyWebb/elmq/pull/53
* chore(main): release 0.6.0 by @caseyWebb in https://github.com/caseyWebb/elmq/pull/54
* fix: drop draft releases, revert to grouped PRs, reset to 0.5.0 by @caseyWebb in https://github.com/caseyWebb/elmq/pull/55


**Full Changelog**: https://github.com/caseyWebb/elmq/compare/v0.5.0...v0.6.0

## 0.6.0 (2026-04-10)

## What's Changed
* fix: use draft releases to support immutable release assets by @caseyWebb in https://github.com/caseyWebb/elmq/pull/33
* feat: add SLSA build provenance attestations for release binaries by @caseyWebb in https://github.com/caseyWebb/elmq/pull/35
* chore(main): release 0.6.0 by @caseyWebb in https://github.com/caseyWebb/elmq/pull/34
* feat: add Claude Code plugin with SessionStart hook and release-please versioning by @caseyWebb in https://github.com/caseyWebb/elmq/pull/36
* fix: reset version to 0.5.0 after orphaned release-please bump by @caseyWebb in https://github.com/caseyWebb/elmq/pull/37
* chore(main): release 0.6.0 by @caseyWebb in https://github.com/caseyWebb/elmq/pull/38
* chore: reset release-please manifest to v0.5.0 by @caseyWebb in https://github.com/caseyWebb/elmq/pull/39
* feat: add release-please multi-package for independent plugin versioning by @caseyWebb in https://github.com/caseyWebb/elmq/pull/41
* chore: release main by @caseyWebb in https://github.com/caseyWebb/elmq/pull/42
* chore: reset release-please manifest to v0.5.0 by @caseyWebb in https://github.com/caseyWebb/elmq/pull/43
* chore: release main by @caseyWebb in https://github.com/caseyWebb/elmq/pull/44
* fix: add force-tag-creation for draft releases by @caseyWebb in https://github.com/caseyWebb/elmq/pull/45
* chore: release main by @caseyWebb in https://github.com/caseyWebb/elmq/pull/46
* fix: unify release-please and release into single workflow by @caseyWebb in https://github.com/caseyWebb/elmq/pull/47
* chore: reset version to 0.5.0 by @caseyWebb in https://github.com/caseyWebb/elmq/pull/49
* chore: release main by @caseyWebb in https://github.com/caseyWebb/elmq/pull/50
* chore: reset version to 0.5.0 by @caseyWebb in https://github.com/caseyWebb/elmq/pull/51
* chore: release main by @caseyWebb in https://github.com/caseyWebb/elmq/pull/52
* fix: use separate release PRs and reset version to 0.5.0 by @caseyWebb in https://github.com/caseyWebb/elmq/pull/53


**Full Changelog**: https://github.com/caseyWebb/elmq/compare/v0.5.0...v0.6.0

## 0.6.0 (2026-04-10)

## What's Changed
* fix: use draft releases to support immutable release assets by @caseyWebb in https://github.com/caseyWebb/elmq/pull/33
* feat: add SLSA build provenance attestations for release binaries by @caseyWebb in https://github.com/caseyWebb/elmq/pull/35
* chore(main): release 0.6.0 by @caseyWebb in https://github.com/caseyWebb/elmq/pull/34
* feat: add Claude Code plugin with SessionStart hook and release-please versioning by @caseyWebb in https://github.com/caseyWebb/elmq/pull/36
* fix: reset version to 0.5.0 after orphaned release-please bump by @caseyWebb in https://github.com/caseyWebb/elmq/pull/37
* chore(main): release 0.6.0 by @caseyWebb in https://github.com/caseyWebb/elmq/pull/38
* chore: reset release-please manifest to v0.5.0 by @caseyWebb in https://github.com/caseyWebb/elmq/pull/39
* feat: add release-please multi-package for independent plugin versioning by @caseyWebb in https://github.com/caseyWebb/elmq/pull/41
* chore: release main by @caseyWebb in https://github.com/caseyWebb/elmq/pull/42
* chore: reset release-please manifest to v0.5.0 by @caseyWebb in https://github.com/caseyWebb/elmq/pull/43
* chore: release main by @caseyWebb in https://github.com/caseyWebb/elmq/pull/44
* fix: add force-tag-creation for draft releases by @caseyWebb in https://github.com/caseyWebb/elmq/pull/45
* chore: release main by @caseyWebb in https://github.com/caseyWebb/elmq/pull/46
* fix: unify release-please and release into single workflow by @caseyWebb in https://github.com/caseyWebb/elmq/pull/47
* chore: reset version to 0.5.0 by @caseyWebb in https://github.com/caseyWebb/elmq/pull/49
* chore: release main by @caseyWebb in https://github.com/caseyWebb/elmq/pull/50
* chore: reset version to 0.5.0 by @caseyWebb in https://github.com/caseyWebb/elmq/pull/51


**Full Changelog**: https://github.com/caseyWebb/elmq/compare/v0.5.0...v0.6.0

## 0.6.0 (2026-04-10)

## What's Changed
* fix: use draft releases to support immutable release assets by @caseyWebb in https://github.com/caseyWebb/elmq/pull/33
* feat: add SLSA build provenance attestations for release binaries by @caseyWebb in https://github.com/caseyWebb/elmq/pull/35
* chore(main): release 0.6.0 by @caseyWebb in https://github.com/caseyWebb/elmq/pull/34
* feat: add Claude Code plugin with SessionStart hook and release-please versioning by @caseyWebb in https://github.com/caseyWebb/elmq/pull/36
* fix: reset version to 0.5.0 after orphaned release-please bump by @caseyWebb in https://github.com/caseyWebb/elmq/pull/37
* chore(main): release 0.6.0 by @caseyWebb in https://github.com/caseyWebb/elmq/pull/38
* chore: reset release-please manifest to v0.5.0 by @caseyWebb in https://github.com/caseyWebb/elmq/pull/39
* feat: add release-please multi-package for independent plugin versioning by @caseyWebb in https://github.com/caseyWebb/elmq/pull/41
* chore: release main by @caseyWebb in https://github.com/caseyWebb/elmq/pull/42
* chore: reset release-please manifest to v0.5.0 by @caseyWebb in https://github.com/caseyWebb/elmq/pull/43
* chore: release main by @caseyWebb in https://github.com/caseyWebb/elmq/pull/44
* fix: add force-tag-creation for draft releases by @caseyWebb in https://github.com/caseyWebb/elmq/pull/45
* chore: release main by @caseyWebb in https://github.com/caseyWebb/elmq/pull/46
* fix: unify release-please and release into single workflow by @caseyWebb in https://github.com/caseyWebb/elmq/pull/47
* chore: reset version to 0.5.0 by @caseyWebb in https://github.com/caseyWebb/elmq/pull/49


**Full Changelog**: https://github.com/caseyWebb/elmq/compare/v0.5.0...v0.6.0

## 0.6.1 (2026-04-10)

## What's Changed
* chore: reset release-please manifest to v0.5.0 by @caseyWebb in https://github.com/caseyWebb/elmq/pull/43
* chore: release main by @caseyWebb in https://github.com/caseyWebb/elmq/pull/44
* fix: add force-tag-creation for draft releases by @caseyWebb in https://github.com/caseyWebb/elmq/pull/45


**Full Changelog**: https://github.com/caseyWebb/elmq/compare/v0.6.0...v0.6.1

## 0.6.0 (2026-04-10)

## What's Changed
* fix: use draft releases to support immutable release assets by @caseyWebb in https://github.com/caseyWebb/elmq/pull/33
* feat: add SLSA build provenance attestations for release binaries by @caseyWebb in https://github.com/caseyWebb/elmq/pull/35
* chore(main): release 0.6.0 by @caseyWebb in https://github.com/caseyWebb/elmq/pull/34
* feat: add Claude Code plugin with SessionStart hook and release-please versioning by @caseyWebb in https://github.com/caseyWebb/elmq/pull/36
* fix: reset version to 0.5.0 after orphaned release-please bump by @caseyWebb in https://github.com/caseyWebb/elmq/pull/37
* chore(main): release 0.6.0 by @caseyWebb in https://github.com/caseyWebb/elmq/pull/38
* chore: reset release-please manifest to v0.5.0 by @caseyWebb in https://github.com/caseyWebb/elmq/pull/39
* feat: add release-please multi-package for independent plugin versioning by @caseyWebb in https://github.com/caseyWebb/elmq/pull/41
* chore: release main by @caseyWebb in https://github.com/caseyWebb/elmq/pull/42


**Full Changelog**: https://github.com/caseyWebb/elmq/compare/v0.5.0...v0.6.0

## 0.6.0 (2026-04-10)

## What's Changed
* fix: use draft releases to support immutable release assets by @caseyWebb in https://github.com/caseyWebb/elmq/pull/33
* feat: add SLSA build provenance attestations for release binaries by @caseyWebb in https://github.com/caseyWebb/elmq/pull/35
* chore(main): release 0.6.0 by @caseyWebb in https://github.com/caseyWebb/elmq/pull/34
* feat: add Claude Code plugin with SessionStart hook and release-please versioning by @caseyWebb in https://github.com/caseyWebb/elmq/pull/36
* fix: reset version to 0.5.0 after orphaned release-please bump by @caseyWebb in https://github.com/caseyWebb/elmq/pull/37
* chore(main): release 0.6.0 by @caseyWebb in https://github.com/caseyWebb/elmq/pull/38
* chore: reset release-please manifest to v0.5.0 by @caseyWebb in https://github.com/caseyWebb/elmq/pull/39
* feat: add release-please multi-package for independent plugin versioning by @caseyWebb in https://github.com/caseyWebb/elmq/pull/41


**Full Changelog**: https://github.com/caseyWebb/elmq/compare/v0.5.0...v0.6.0

## 0.6.0 (2026-04-10)

## What's Changed
* fix: use draft releases to support immutable release assets by @caseyWebb in https://github.com/caseyWebb/elmq/pull/33
* feat: add SLSA build provenance attestations for release binaries by @caseyWebb in https://github.com/caseyWebb/elmq/pull/35
* chore(main): release 0.6.0 by @caseyWebb in https://github.com/caseyWebb/elmq/pull/34
* feat: add Claude Code plugin with SessionStart hook for elmq guidance by @caseyWebb in https://github.com/caseyWebb/elmq/pull/36
* fix: reset version to 0.5.0 after orphaned release-please bump by @caseyWebb in https://github.com/caseyWebb/elmq/pull/37


**Full Changelog**: https://github.com/caseyWebb/elmq/compare/v0.5.0...v0.6.0

## 0.6.0 (2026-04-10)

## What's Changed
* fix: use draft releases to support immutable release assets by @caseyWebb in https://github.com/caseyWebb/elmq/pull/33
* feat: add SLSA build provenance attestations for release binaries by @caseyWebb in https://github.com/caseyWebb/elmq/pull/35


**Full Changelog**: https://github.com/caseyWebb/elmq/compare/v0.5.0...v0.6.0

## 0.5.0 (2026-04-10)

## What's Changed
* fix: move-decl constructor exposure + benchmark guide optimization by @caseyWebb in https://github.com/caseyWebb/elmq/pull/27
* chore(deps): Bump tree-sitter from 0.26.7 to 0.26.8 by @dependabot[bot] in https://github.com/caseyWebb/elmq/pull/30
* chore(deps): Bump dependabot/fetch-metadata from 2 to 3 by @dependabot[bot] in https://github.com/caseyWebb/elmq/pull/29
* docs(benchmarks): refine elmq-guide and update results by @caseyWebb in https://github.com/caseyWebb/elmq/pull/31
* feat: add npm distribution for cross-platform installs by @caseyWebb in https://github.com/caseyWebb/elmq/pull/32


**Full Changelog**: https://github.com/caseyWebb/elmq/compare/v0.4.0...v0.5.0

## 0.4.0 (2026-04-10)

## What's Changed
* chore(deps): Bump tree-sitter from 0.26.7 to 0.26.8 by @dependabot[bot] in https://github.com/caseyWebb/elmq/pull/23
* chore(deps): Bump tokio from 1.50.0 to 1.51.0 by @dependabot[bot] in https://github.com/caseyWebb/elmq/pull/24
* refactor!: drop mcp server; add oracle benchmark treatment arm by @caseyWebb in https://github.com/caseyWebb/elmq/pull/22
* feat: agent optimization — CLI enhancements, benchmark harness, and elmq grep by @caseyWebb in https://github.com/caseyWebb/elmq/pull/26


**Full Changelog**: https://github.com/caseyWebb/elmq/compare/v0.3.0...v0.4.0

## 0.3.0 (2026-03-28)

## What's Changed
* fix: code scanning alert no. 1: Workflow does not contain permissions by @caseyWebb in https://github.com/caseyWebb/elmq/pull/14
* feat: add `mv` command for project-wide module renaming by @caseyWebb in https://github.com/caseyWebb/elmq/pull/16
* feat: add `refs` command for project-wide reference lookup by @caseyWebb in https://github.com/caseyWebb/elmq/pull/17
* feat: add `rename` command for project-wide declaration renaming by @caseyWebb in https://github.com/caseyWebb/elmq/pull/18
* docs: update docs for `rename` command by @caseyWebb in https://github.com/caseyWebb/elmq/pull/19
* feat: add `move-decl` command for moving declarations between modules by @caseyWebb in https://github.com/caseyWebb/elmq/pull/20
* feat: add `variant` command for adding/removing type constructors with case propagation by @caseyWebb in https://github.com/caseyWebb/elmq/pull/21


**Full Changelog**: https://github.com/caseyWebb/elmq/compare/v0.2.2...v0.3.0

## 0.2.2 (2026-03-27)

## What's Changed
* fix: update deprecated CI runners and actions by @caseyWebb in https://github.com/caseyWebb/elmq/pull/4
* chore: add Dependabot for Cargo and GitHub Actions by @caseyWebb in https://github.com/caseyWebb/elmq/pull/5
* chore(deps): Bump jdx/mise-action from 2 to 4 by @dependabot[bot] in https://github.com/caseyWebb/elmq/pull/9
* fix: replace mise with rust-toolchain.toml by @caseyWebb in https://github.com/caseyWebb/elmq/pull/11
* ci: add Cargo caching with Swatinem/rust-cache by @caseyWebb in https://github.com/caseyWebb/elmq/pull/12
* chore: auto-merge non-major Dependabot PRs by @caseyWebb in https://github.com/caseyWebb/elmq/pull/10
* chore(deps): Bump actions/checkout from 4 to 6 by @dependabot[bot] in https://github.com/caseyWebb/elmq/pull/7
* chore(deps): Bump rmcp from 0.11.0 to 1.3.0 by @dependabot[bot] in https://github.com/caseyWebb/elmq/pull/8
* fix: handle path validation consistently on Windows by @caseyWebb in https://github.com/caseyWebb/elmq/pull/13

## New Contributors
* @dependabot[bot] made their first contribution in https://github.com/caseyWebb/elmq/pull/9

**Full Changelog**: https://github.com/caseyWebb/elmq/compare/v0.2.1...v0.2.2

## 0.2.1 (2026-03-27)

## What's Changed
* fix: use RELEASE_TOKEN PAT, fix CI, bump rust to 1.94.1 by @caseyWebb in https://github.com/caseyWebb/elmq/pull/2

## New Contributors
* @caseyWebb made their first contribution in https://github.com/caseyWebb/elmq/pull/2

**Full Changelog**: https://github.com/caseyWebb/elmq/compare/v0.2.0...v0.2.1

## 0.2.0 (2026-03-27)

**Full Changelog**: https://github.com/caseyWebb/elmq/compare/v0.1.0...v0.2.0
