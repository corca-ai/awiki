# Release

## CI

GitHub Actions defines two repository workflows:

- `.github/workflows/ci.yml` for test and lint on pushes to `main` and pull requests
- `.github/workflows/release.yml` for tagged releases

## Tagged Release Flow

Pushing a `v*` tag triggers [GoReleaser](https://goreleaser.com/):

```sh
git tag v0.1.0
git push origin v0.1.0
```

GoReleaser:

- cross-compiles for macOS, Linux, and Windows
- creates release archives and `checksums.txt`
- publishes a GitHub Release
- updates the Homebrew tap

## Repository Assumptions

Current release configuration assumes:

- GitHub repository: `corca-ai/awiki`
- Homebrew tap: `corca-ai/homebrew-tap`

If those change, update `.goreleaser.yaml` and `install.sh`.

## Homebrew

The Homebrew formula installs the `awiki` binary and verifies it with:

```sh
awiki version
```

## Related Docs

- [Install](install.md) — installation paths users consume
- [Testing](testing.md) — CI checks before release
- [Build & Run](build.md) — local build commands used during release preparation
