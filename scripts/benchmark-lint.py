#!/usr/bin/env python3
"""Measure end-to-end lint on a wiki with repeated source references."""
import argparse
import json
from pathlib import Path
import statistics
import subprocess
import tempfile
import time


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path)
    parser.add_argument("--rounds", type=int, default=3)
    args = parser.parse_args()
    if args.rounds < 1:
        parser.error("--rounds must be positive")
    binary = args.binary.resolve()
    with tempfile.TemporaryDirectory(prefix="awiki-benchmark-") as temporary:
        root = Path(temporary)
        docs = root / "docs"
        docs.mkdir()
        methods = "".join(f"method{i}() {{ return {i}; }}\n" for i in range(200))
        (root / "Foo.ts").write_text("export class Foo {\n" + methods + "}\n")
        config = '[lint.rules]\nbroken-code-reference="error"\n'
        config += (
            '[[lint.overrides]]\nfiles=["*.md"]\n'
            '[lint.overrides.rules]\norphan="off"\n'
        ) * 10
        (docs / "awiki.toml").write_text(config)
        for index in range(100):
            body = f"Summary. See [next](Note{(index + 1) % 100}.md).\n\n"
            body += "".join(
                f"See [method](../Foo.ts#Foo.method{i}).\n" for i in range(10)
            )
            (docs / f"Note{index}.md").write_text(body)
        command = [str(binary), "lint", "-root", str(docs)]
        subprocess.run(command, check=True, stdout=subprocess.DEVNULL)
        samples = []
        for _ in range(args.rounds):
            start = time.perf_counter()
            subprocess.run(command, check=True, stdout=subprocess.DEVNULL)
            samples.append(time.perf_counter() - start)
        print(json.dumps({
            "documents": 100, "code_references": 1000, "source_methods": 200,
            "overrides": 10, "rounds": args.rounds, "seconds": samples,
            "median_seconds": statistics.median(samples),
        }))


if __name__ == "__main__":
    main()
