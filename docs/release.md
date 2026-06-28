# Release

## CI

GitHub Actions defines two repository workflows:

- `.github/workflows/ci.yml` for test and lint on pushes to `main` and pull requests
- `.github/workflows/release.yml` for tagged releases

## Tagged Release Flow

Pushing a SemVer tag triggers the cargo-dist release workflow:

```sh
git tag v0.1.0
git push origin v0.1.0
```

The release workflow:

- runs the repository CI quality gate
- builds release archives for macOS and Linux targets
- creates shell and Homebrew installers
- publishes a GitHub Release
- pushes the Homebrew formula to `corca-ai/homebrew-tap`

## Repository Assumptions

Current release configuration assumes:

- GitHub repository: `corca-ai/awiki`
- Homebrew tap: `corca-ai/homebrew-tap`
- Homebrew formula: `awiki`
- CI secret: `HOMEBREW_TAP_TOKEN` with push access to the tap

If those change, update `dist-workspace.toml`, `.github/workflows/release.yml`,
and `install.sh`.

## Homebrew

The Homebrew formula installs the `awiki` binary and verifies it with:

```sh
awiki version
```

## Related Docs

- [Install](install.md) — installation paths users consume
- [Testing](testing.md) — CI checks before release
- [Build & Run](build.md) — local build commands used during release preparation
