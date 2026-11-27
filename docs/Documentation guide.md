This document defines how to write and maintain project docs.

## Goal

Keep docs easy to scan and easy to trust for both humans and agents.

## Rules

- Treat docs like code.
- Keep intent explicit and concise.
- Prefer small focused docs over long mixed docs.
- Use Wikipedia naming conventions, e.g., `docs/Core features.md`.
- Use a flat structure without subdirs.
- Keep `README.md` minimal and point to `AGENTS.md`.
- Keep `AGENTS.md` minimal and link to detailed docs.
- Always run `awiki lint docs/` and fix all issues.
- Keep explanations brief and link to the code, e.g.:

  ```md
  [`blah()`](../src/Foo.ts#blah)
  [`Foo.blah()`](../src/Foo.ts#Foo.blah)
  [`Vault::load()`](../src/wiki/load.rs#Vault::load)
  ```

## Structure

A document's name serves as its title. The first paragraph should summarize the
entire document. Follow the summary paragraph with sections in the form `## Foo`.
Put links naturally into sentences so readers understand the context.

Use sentence case and spaces in filenames. Enclose destinations containing
spaces in angle brackets, as in `[Wiki model](<Wiki model.md>)`.
See [Testing](Testing.md) for the local toolchain and documentation checks.
