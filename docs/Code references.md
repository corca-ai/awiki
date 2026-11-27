Code references are ordinary Markdown links whose fragments name declarations in TypeScript, TSX, or Rust files; awiki can fail lint when the file, syntax, or declaration is invalid.

## Syntax

Write the source path relative to the referring document and put the declaration
name after `#`. Names are case-sensitive. Display text is free-form; omit `()`
from fragments even when the label shows a function call.

```md
[`blah()`](../src/Foo.ts#blah)
[`Foo`](../src/Foo.ts#Foo)
[`Foo.blah()`](../src/Foo.ts#Foo.blah)
[`Vault::load()`](../src/wiki/load.rs#Vault::load)
```

The last example uses this repository's
[`Vault::load()`](../src/wiki/load.rs#Vault::load) declaration. Paths may leave
the wiki root, and destinations containing spaces use angle brackets.

## TypeScript and TSX

Use `.` between containing declarations and their members: `#Foo.blah`.
Supported `.ts` and `.tsx` targets include top-level functions, variables,
classes, interfaces, types, enums, namespaces, and directly declared class or
interface members. Overloads share a declaration name.

Inherited members, re-exported aliases, function locals, inferred object
properties, and type-based resolution are outside the check's scope.

## Rust

Use `::` between modules and members: `#Vault::load`. Supported `.rs` targets
include functions, structs, enums and variants, traits, type aliases,
constants, statics, named fields, modules, macro definitions, and directly
declared associated items. Generic impls use the type name without arguments.

Inline modules qualify their contents. Link external module files directly.
Trait impl members appear under the implementing type's name. There is no
import resolution, type checking, macro expansion, or conditional-compilation
evaluation.

## Validation

Enable the rule through [Lint configuration](<Lint configuration.md>):

```toml
[lint.rules]
broken-code-reference = "error"
```

[`SourceCache::check()`](../src/wiki/source.rs#SourceCache::check) parses each
target file once per lint run and looks up its declarations. The next command
loads fresh files. Missing or unreadable files, unsupported
languages, syntax errors, and missing names produce distinct diagnostics.
Code references in fenced examples, inline code, images, and external links
are ignored. Only body links participate, not front matter links.

Source files do not become [wiki graph](<Wiki model.md>) nodes. These fragments
validate references; they do not add navigation support to GitHub or editors.
