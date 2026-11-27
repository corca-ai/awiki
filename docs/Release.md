awiki releases run repository quality gates, package binaries for macOS and Linux, and publish GitHub assets, installers, checksums, and a Homebrew formula from a version tag.

## Preparation

Follow [Testing](Testing.md) before releasing and update the package version in
`Cargo.toml` and `Cargo.lock`. Build and test with the pinned Rust toolchain
from [Build and run](<Build and run.md>). Confirm the tag matches the package
version before pushing it.

## Publishing

The [release workflow](../.github/workflows/release.yml) runs for version tags.
It checks the repository, builds Apple Silicon/Intel macOS and ARM64/x86-64
Linux archives, creates installers and checksums, publishes a GitHub Release,
and updates `corca-ai/homebrew-tap`.

For example, after setting the package version to `0.8.0`:

```sh
git tag v0.8.0
git push origin v0.8.0
```

The [dist configuration](../dist-workspace.toml) defines targets and installers.
Apple Silicon builds explicitly use `macos-15`. The workflow requires
`HOMEBREW_TAP_TOKEN` with push access to the tap.

## Verification

Confirm the workflow succeeds, verify artifact checksums and `awiki version`,
and check that the Homebrew formula points to the new version. Published tags
must remain fixed; correct a published release with a new version.

[Installation](Installation.md) describes the resulting user install paths.
