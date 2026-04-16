# Testing

## Unit Tests

```sh
./scripts/test.sh
```

The test suite covers command behavior, parsing, graph analysis, and rename
rewrites.

## Lint

The project uses [golangci-lint](https://golangci-lint.run/) with the
configuration in `.golangci.yml`.

```sh
./scripts/lint.sh
```

Enabled linters include:

- `errcheck`
- `govet`
- `staticcheck`
- `unused`
- `ineffassign`
- `gocritic`
- `gocognit`
- `bodyclose`
- `nilerr`
- `errorlint`
- `unparam`
- `unconvert`

## Pre-commit Hook

Enable the repository hook once after cloning:

```sh
git config core.hooksPath .githooks
```

The pre-commit hook runs:

- `./scripts/test.sh`
- `./scripts/lint.sh`
- `AWIKI_BUILD_OUTPUT=.tmp-bin/awiki AWIKI_SKIP_LINK=1 ./scripts/build.sh`

## CI

GitHub Actions runs on every push to `main` and on pull requests.
The CI workflow installs the Go version declared in `go.mod`.

The CI job executes:

1. `go mod verify`
2. `go mod tidy && git diff --exit-code go.mod go.sum`
3. `CGO_ENABLED=1 go test -race ./...`
4. `golangci-lint run ./...`

## Related Docs

- [Build & Run](build.md) — local build commands
- [Release](release.md) — tagged release flow
