#!/usr/bin/env python3
from __future__ import annotations

import difflib
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
BINARY = ROOT / "target" / "debug" / "pasta"
FIXTURE_ROOT = ROOT / "tests" / "fixtures"


@dataclass(frozen=True)
class Case:
    name: str
    script: Path
    expected_stdout: str
    expected_stderr: str
    expected_exit: int


def read_text(path: Path) -> str:
    return path.read_text(encoding="utf-8") if path.exists() else ""


def discover_cases(directory: Path) -> list[Case]:
    if not directory.exists():
        return []

    cases: list[Case] = []
    for script in sorted(directory.glob("*.ps")):
        cases.append(
            Case(
                name=script.relative_to(ROOT).as_posix(),
                script=script,
                expected_stdout=read_text(script.with_suffix(".stdout")),
                expected_stderr=read_text(script.with_suffix(".stderr")),
                expected_exit=int(read_text(script.with_suffix(".exit")).strip() or "0")
                if script.with_suffix(".exit").exists()
                else 0,
            )
        )
    return cases


def run(cmd: list[str]) -> None:
    subprocess.run(cmd, cwd=ROOT, check=True)


def build_binary() -> None:
    run(["cargo", "build", "--quiet", "--bin", "pasta"])


def diff(label: str, expected: str, actual: str) -> str:
    lines = difflib.unified_diff(
        expected.splitlines(keepends=True),
        actual.splitlines(keepends=True),
        fromfile=f"expected {label}",
        tofile=f"actual {label}",
    )
    return "".join(lines)


def run_case(case: Case) -> list[str]:
    proc = subprocess.run(
        [str(BINARY), str(case.script)],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )

    failures: list[str] = []
    if proc.returncode != case.expected_exit:
        failures.append(
            f"{case.name}: expected exit {case.expected_exit}, got {proc.returncode}"
        )
    if proc.stdout != case.expected_stdout:
        failures.append(f"{case.name}: stdout mismatch\n{diff('stdout', case.expected_stdout, proc.stdout)}")
    if proc.stderr != case.expected_stderr:
        failures.append(f"{case.name}: stderr mismatch\n{diff('stderr', case.expected_stderr, proc.stderr)}")
    return failures


def run_fixture_group(name: str, directories: list[Path]) -> None:
    cases: list[Case] = []
    for directory in directories:
        cases.extend(discover_cases(directory))

    if not cases:
        raise SystemExit(f"no fixture cases found for {name}")

    build_binary()

    failures: list[str] = []
    for case in cases:
        failures.extend(run_case(case))

    if failures:
        print(f"[{name}] failed ({len(failures)} issue(s))", file=sys.stderr)
        for failure in failures:
            print(failure, file=sys.stderr)
        raise SystemExit(1)

    print(f"[{name}] passed {len(cases)} fixture case(s)")


def smoke() -> None:
    run(["cargo", "test", "--quiet", "--test", "cli_smoke"])
    run_fixture_group("smoke", [FIXTURE_ROOT / "smoke"])


def golden_check() -> None:
    run(["cargo", "test", "--quiet", "--test", "cli_smoke", "--test", "graphics_grid_api"])
    run_fixture_group(
        "golden-check",
        [FIXTURE_ROOT / "smoke", FIXTURE_ROOT / "golden"],
    )


def main(argv: list[str]) -> int:
    if len(argv) != 2 or argv[1] not in {"smoke", "golden-check"}:
        print("usage: tools/devkit/tasks.py [smoke|golden-check]", file=sys.stderr)
        return 2

    if argv[1] == "smoke":
        smoke()
    else:
        golden_check()
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
