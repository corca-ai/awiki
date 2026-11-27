Format and rename build a complete change plan, support previews without writes, stage replacement files before editing originals, and restore completed writes when an ordinary I/O failure interrupts the operation.

## Preview

```sh
awiki format -root ~/wiki --dry-run
awiki rename -root ~/wiki --dry-run "Old title" "New title"
```

Dry runs print `// dry_run changes=<n>`, source/destination filenames, and the
normal operation summary. They do not write files or create directories.
[`Vault::plan_format()`](../src/wiki/format.rs#Vault::plan_format) and
[`Vault::plan_rename()`](../src/wiki/rename.rs#Vault::plan_rename) prepare the plans.
See the [Command-line interface](<Command-line interface.md>) for other options.

## Applying a plan

[`ChangePlan::apply()`](../src/wiki/changes.rs#ChangePlan::apply) checks source
snapshots, rejects conflicting destinations, and prepares temporary replacements
and backups before committing. Replacements preserve file permissions and use
same-directory atomic replacements. New destinations are published without
replacing existing files. Destination directories must stay inside the
wiki, including through symlinks. Read-only sources are rejected, and source text is checked again before each
write to detect changes since planning.

On a commit error, completed writes are restored in reverse order. Newly created
empty directories and temporary files are cleaned up. If an external edit or
another I/O error prevents restoration, the error identifies a retained recovery
copy; external content is not silently overwritten during recovery.

## Limits

Each file replacement is atomic, but a whole-wiki edit is not atomic to other
processes. External edits can still race the source checks and replacements. Automatic rollback handles errors returned during this process;
process termination, power loss, and filesystem failure can interrupt recovery.
Keep wiki files in version control and review the plan before large edits.
[Testing](Testing.md) describes the failure and preservation regression cases.
