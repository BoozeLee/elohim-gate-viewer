#!/usr/bin/env python3
"""Publish a gate run as a static, read-only report.

This is the only place local paths are removed. It runs once, before anything is
written, and it refuses to write at all if a path survives -- so a leak cannot
reach a public URL by being missed in one field of one payload.

Two rules, because one rule cannot be right for both cases
    A path may contain spaces ("/home/example/My Docs/instrument.py"), and prose
    after a path does not belong to it ("/home/example/i.py for details"). No single
    regular expression can tell those apart. So fields whose *value is* a path
    are reduced to their file name outright, and every other string gets a
    whitespace-exclusive sweep as a net for the fields nobody listed.

What is kept
    Everything else: skill names, claim ids, expected/measured pairs, residuals,
    sha256 values and the per-skill `seal`. Those are the measurement, and the
    point of the page is that a reader can check them. Only the directory is
    removed, because "/home/example/..." is identity and "summoning_shard.py" is
    content.

Usage
    tools/publish_report.py GATE_PAYLOAD.json [OUTPUT.json]

    Reads stdin when no path is given. Writes to
    public/data/latest.json by default.
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import pathlib
import re
import sys

SCHEMA_PREFIX = "elohim.gate/"

# A rooted path token inside a prose string. The lookbehind is what keeps this
# from mangling the two things that legitimately contain a slash: URLs
# ("https://host/x" -- the first slash follows a colon, the second a slash, the
# third a word character) and ratios ("4/5" -- the slash follows a digit).
ROOTED = re.compile(r"""(?<![\w:/.\-@])/[^\s"'`,;)\]}<>]+""")

# Field names whose value is a path rather than prose about a path.
PATH_KEYS = frozenset(
    {
        "path",
        "instrument",
        "instrument_path",
        "file",
        "filename",
        "dir",
        "directory",
        "cwd",
        "root",
        "source_path",
    }
)

TRAILING = ".,;:)]}\"'"


def _strip_trailing(text: str) -> tuple[str, str]:
    trail = ""
    while text and text[-1] in TRAILING:
        trail = text[-1] + trail
        text = text[:-1]
    return text, trail


def basename(text: str) -> str:
    """File name of a whole path value, prose punctuation kept."""
    body, trail = _strip_trailing(text.replace("\\", "/"))
    cut = body.rfind("/")
    return (body[cut + 1 :] if cut != -1 else body) + trail


class Scrubber:
    def __init__(self) -> None:
        self.count = 0
        self.fields = 0

    def _prose(self, text: str) -> str:
        def sub(match: re.Match) -> str:
            self.count += 1
            return basename(match.group(0))

        return ROOTED.sub(sub, text)

    def scrub(self, node, key: str | None = None):
        if isinstance(node, str):
            if key in PATH_KEYS:
                # The value is a path, so whitespace is part of it rather than
                # the end of it. Only record a redaction if something was lost.
                reduced = basename(node)
                if reduced != node:
                    self.count += 1
                    self.fields += 1
                return reduced
            return self._prose(node)
        if isinstance(node, list):
            return [self.scrub(item) for item in node]
        if isinstance(node, dict):
            return {k: self.scrub(v, k) for k, v in node.items()}
        return node


def residual_paths(document) -> list[str]:
    """Every rooted path token still present, as evidence for a refusal."""
    found: list[str] = []

    def walk(node) -> None:
        if isinstance(node, str):
            found.extend(m.group(0) for m in ROOTED.finditer(node))
        elif isinstance(node, list):
            for item in node:
                walk(item)
        elif isinstance(node, dict):
            for value in node.values():
                walk(value)

    walk(document)
    return found


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("input", nargs="?", help="gate payload JSON; stdin if omitted")
    parser.add_argument(
        "-o",
        "--output",
        default="public/data/latest.json",
        help="where to write (default: %(default)s)",
    )
    args = parser.parse_args(argv)

    try:
        text = sys.stdin.read() if args.input is None else pathlib.Path(args.input).read_text()
    except OSError as err:
        print(f"cannot read payload: {err}", file=sys.stderr)
        return 2

    try:
        payload = json.loads(text)
    except json.JSONDecodeError as err:
        print(f"payload is not JSON: {err}", file=sys.stderr)
        return 2

    schema = payload.get("schema") if isinstance(payload, dict) else None
    if not isinstance(schema, str) or not schema.startswith(SCHEMA_PREFIX):
        print(f"refusing to publish: schema is {schema!r}, expected {SCHEMA_PREFIX}*", file=sys.stderr)
        return 2

    scrubber = Scrubber()
    cleaned = scrubber.scrub(payload)

    survivors = residual_paths(cleaned)
    if survivors:
        print("refusing to publish: a path survived scrubbing", file=sys.stderr)
        for token in survivors[:10]:
            print(f"  {token}", file=sys.stderr)
        return 1

    document = {
        "published": dt.datetime.now(dt.timezone.utc).replace(microsecond=0).isoformat(),
        "source_run": payload.get("run"),
        "redacted_paths": scrubber.count,
        "redacted_from_path_fields": scrubber.fields,
        "scrubber": "tools/publish_report.py",
        "payload": cleaned,
    }

    out = pathlib.Path(args.output)
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(document, indent=2, ensure_ascii=False) + "\n")
    print(f"wrote {out} ({scrubber.count} path(s) redacted, {scrubber.fields} from path fields)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())