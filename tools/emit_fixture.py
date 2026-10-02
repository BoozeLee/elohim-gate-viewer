#!/usr/bin/env python3
"""Print a gate payload fixture on stdout, so the desktop app can be shown one.

Why this exists instead of `cat`:

The app appends `--all --json` to whatever ELOHIM_GATE_CMD names, because the real
gate is a harness that takes those flags. Measured: `cat payload_fail.json --all
--json` exits 1 and prints nothing, so the window reports "cat exited 1 without
printing a payload" and renders the unavailable card rather than the fixture. A
program that does not understand the flags cannot be used as an override at all.

That is why the window screenshots were not reproducible. They could only be
taken against a real gate run, which meant `shots/window-pass.png` required the
gate to pass on the day it was captured -- and it does not pass today. Reading a
checked-in payload instead makes both shots reproducible from the committed tree,
which is also how `shots/pass.png` and `shots/fail.png` have always been made.

Usage:
    ELOHIM_GATE_CMD="python3 tools/emit_fixture.py <fixture.json>" tools/see-window.sh out.png

The flags are accepted and ignored. The payload is validated before printing, so
a wrong path fails here with a readable message instead of surfacing later as a
window that renders nothing -- the same failure, one step further from its cause.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

# Matches KNOWN_SCHEMAS in src-tauri/src/gate.rs. A payload outside this set is
# rejected by the app with UnknownSchema, which is a less obvious message than
# saying so here.
KNOWN_SCHEMAS = ("elohim.gate/1",)


def main(argv: list[str]) -> int:
    positional = [arg for arg in argv if not arg.startswith("-")]

    if not positional:
        print(f"usage: {Path(sys.argv[0]).name} <payload.json>", file=sys.stderr)
        return 2

    path = Path(positional[0])
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except OSError as error:
        print(f"cannot read {path}: {error}", file=sys.stderr)
        return 2
    except json.JSONDecodeError as error:
        print(f"{path} is not valid JSON: {error}", file=sys.stderr)
        return 2

    if not isinstance(payload, dict):
        print(f"{path} holds a {type(payload).__name__}, not an object", file=sys.stderr)
        return 2

    schema = payload.get("schema")
    if schema not in KNOWN_SCHEMAS:
        print(
            f"{path} has schema {schema!r}; expected one of {list(KNOWN_SCHEMAS)}",
            file=sys.stderr,
        )
        return 2

    sys.stdout.write(json.dumps(payload))

    # Mirror the real harness on the exit code, so a FAIL payload exits non-zero
    # exactly as a real failing run does. Measured on harness_run.py: the
    # aggregate is `"PASS" if code == EXIT_OK else "FAIL"` over the per-skill
    # verdicts, and a non-zero exit is what the window's own status line reports.
    # Without this, `window-fail.png` would show a FAIL verdict beside "exit 0",
    # which is a state the real gate cannot produce and would make the committed
    # screenshot evidence for the exit code worthless.
    failed = [
        skill.get("skill")
        for skill in payload.get("skills", [])
        if isinstance(skill, dict) and skill.get("verdict") == "FAIL"
    ]
    return 1 if failed else 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))