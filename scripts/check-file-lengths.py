#!/usr/bin/env python3
"""Ratcheting Rust file-length gate.

The committed budget records existing files above the default ceiling. A budgeted
file may not grow, and if it shrinks its budget must shrink with it. Unbudgeted
Rust files must stay under the default ceiling.
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
import tempfile
from pathlib import Path


DEFAULT_CONFIG = Path("scripts/file-length-budgets.json")
DEFAULT_ROOTS = ("src",)


def count_lines(path: Path) -> int:
    with path.open("rb") as handle:
        return sum(1 for _ in handle)


def parse_budget_json(text: str, source: str) -> tuple[int, dict[str, int]]:
    try:
        raw = json.loads(text)
    except json.JSONDecodeError as err:
        raise SystemExit(f"invalid JSON in {source}: {err}") from None

    default_max = raw.get("default_max_lines")
    files = raw.get("files")
    if not isinstance(default_max, int) or default_max <= 0:
        raise SystemExit(f"{source}: default_max_lines must be a positive integer")
    if not isinstance(files, dict):
        raise SystemExit(f"{source}: files must be an object mapping paths to line budgets")

    budgets: dict[str, int] = {}
    for file_path, budget in files.items():
        if not isinstance(file_path, str):
            raise SystemExit(f"{source}: budget paths must be strings")
        if not isinstance(budget, int) or budget <= 0:
            raise SystemExit(f"{source}: budget for {file_path} must be a positive integer")
        if budget <= default_max:
            raise SystemExit(
                f"{source}: remove {file_path}; budgets are only for files above {default_max} lines"
            )
        budgets[file_path] = budget
    return default_max, budgets


def load_budget(path: Path) -> tuple[int, dict[str, int]]:
    try:
        text = path.read_text(encoding="utf-8")
    except FileNotFoundError:
        raise SystemExit(f"missing file-length budget: {path}") from None
    return parse_budget_json(text, str(path))


def rust_files(root: Path, scan_roots: list[str]) -> list[Path]:
    files: list[Path] = []
    for scan_root in scan_roots:
        base = root / scan_root
        if not base.exists():
            raise SystemExit(f"scan root does not exist: {scan_root}")
        files.extend(path for path in base.rglob("*.rs") if path.is_file())
    return sorted(files)


def relpath(path: Path, root: Path) -> str:
    return path.relative_to(root).as_posix()


def run_git(root: Path, args: list[str], *, allow_failure: bool = False) -> str | None:
    result = subprocess.run(
        ["git", "-C", str(root), *args],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    if result.returncode == 0:
        return result.stdout
    if allow_failure:
        return None
    command = " ".join(["git", *args])
    raise SystemExit(f"{command} failed: {result.stderr.strip()}")


def base_budget(
    root: Path, config: Path, ratchet_base: str
) -> tuple[int, dict[str, int]] | None:
    try:
        config_rel = relpath(config.resolve(), root)
    except ValueError:
        raise SystemExit(f"budget config must be under repository root: {config}") from None

    merge_base = run_git(root, ["merge-base", "HEAD", ratchet_base])
    assert merge_base is not None
    base_commit = merge_base.strip()
    text = run_git(root, ["show", f"{base_commit}:{config_rel}"], allow_failure=True)
    if text is None:
        return None
    return parse_budget_json(text, f"{ratchet_base}:{config_rel}")


def no_loosening_failures(
    default_max: int,
    budgets: dict[str, int],
    old_default_max: int,
    old_budgets: dict[str, int],
) -> list[str]:
    failures: list[str] = []
    if default_max > old_default_max:
        failures.append(
            f"default_max_lines increased {old_default_max} -> {default_max}; ratchets may only tighten"
        )

    for file_path, budget in sorted(budgets.items()):
        old_budget = old_budgets.get(file_path)
        if old_budget is None:
            failures.append(
                f"{file_path}: new over-target budget {budget}; split the file under {default_max} lines"
            )
        elif budget > old_budget:
            failures.append(
                f"{file_path}: budget increased {old_budget} -> {budget}; ratchets may only tighten"
            )
    return failures


def check(
    root: Path,
    config: Path,
    scan_roots: list[str],
    *,
    quiet: bool = False,
    ratchet_base: str | None = None,
) -> int:
    default_max, budgets = load_budget(config)
    files = rust_files(root, scan_roots)
    actual = {relpath(path, root): count_lines(path) for path in files}

    failures: list[str] = []

    for file_path, budget in sorted(budgets.items()):
        lines = actual.get(file_path)
        if lines is None:
            failures.append(f"stale budget: {file_path} no longer exists")
        elif lines > budget:
            failures.append(f"{file_path}: {lines} lines exceeds budget {budget}")
        elif lines <= default_max:
            failures.append(
                f"{file_path}: {lines} lines is at or below {default_max}; remove its budget entry"
            )
        elif lines < budget:
            failures.append(f"{file_path}: lower budget {budget} -> {lines}")

    for file_path, lines in sorted(actual.items()):
        if file_path not in budgets and lines > default_max:
            failures.append(
                f"{file_path}: {lines} lines exceeds default limit {default_max}; split it"
            )

    if ratchet_base is not None:
        old_budget = base_budget(root, config, ratchet_base)
        if old_budget is not None:
            old_default_max, old_budgets = old_budget
            failures.extend(
                no_loosening_failures(default_max, budgets, old_default_max, old_budgets)
            )

    if failures:
        if quiet:
            return 1
        print("file-length gate failed:", file=sys.stderr)
        for failure in failures:
            print(f"  - {failure}", file=sys.stderr)
        return 1

    if not quiet:
        print(
            f"file-length gate: {len(files)} Rust files checked; "
            f"default max {default_max}; {len(budgets)} ratcheted budgets"
        )
    return 0


def self_test() -> int:
    with tempfile.TemporaryDirectory() as temp_dir:
        root = Path(temp_dir)
        (root / "src").mkdir(parents=True)
        config = root / "scripts/file-length-budgets.json"
        config.parent.mkdir()

        small = root / "src/lib.rs"
        large = root / "src/large.rs"
        small.write_text("fn ok() {}\n", encoding="utf-8")
        large.write_text("fn line() {}\n" * 4, encoding="utf-8")
        config.write_text(
            json.dumps({"default_max_lines": 3, "files": {"src/large.rs": 4}}),
            encoding="utf-8",
        )
        if check(root, config, ["src"], quiet=True) != 0:
            print("self-test failed: valid ratchet rejected", file=sys.stderr)
            return 1

        large.write_text("fn line() {}\n" * 5, encoding="utf-8")
        if check(root, config, ["src"], quiet=True) == 0:
            print("self-test failed: growth was accepted", file=sys.stderr)
            return 1

        config.write_text(
            json.dumps({"default_max_lines": 3, "files": {"src/large.rs": 5}}),
            encoding="utf-8",
        )
        large.write_text("fn line() {}\n" * 4, encoding="utf-8")
        if check(root, config, ["src"], quiet=True) == 0:
            print("self-test failed: stale shrink budget was accepted", file=sys.stderr)
            return 1

        large.write_text("fn line() {}\n" * 3, encoding="utf-8")
        if check(root, config, ["src"], quiet=True) == 0:
            print("self-test failed: stale budget was accepted", file=sys.stderr)
            return 1

        ratchet_failures = no_loosening_failures(
            4,
            {"src/large.rs": 6, "src/new.rs": 4},
            3,
            {"src/large.rs": 5},
        )
        if len(ratchet_failures) != 3:
            print("self-test failed: budget loosening was not fully rejected", file=sys.stderr)
            return 1
        if no_loosening_failures(2, {"src/large.rs": 4}, 3, {"src/large.rs": 5}):
            print("self-test failed: budget tightening was rejected", file=sys.stderr)
            return 1

    print("file-length gate self-test passed")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--config", type=Path, default=DEFAULT_CONFIG)
    parser.add_argument("--root", type=Path, default=Path("."))
    parser.add_argument("--scan-root", action="append", dest="scan_roots")
    parser.add_argument(
        "--ratchet-base",
        help="git ref whose budget config is the no-loosening baseline",
    )
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()

    if args.self_test:
        return self_test()

    root = args.root.resolve()
    config = args.config
    if not config.is_absolute():
        config = root / config
    scan_roots = args.scan_roots or list(DEFAULT_ROOTS)
    return check(root, config, scan_roots, ratchet_base=args.ratchet_base)


if __name__ == "__main__":
    sys.exit(main())
