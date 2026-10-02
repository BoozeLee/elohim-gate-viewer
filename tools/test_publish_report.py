#!/usr/bin/env python3
"""Tests for tools/publish_report.py.

The load-bearing one is `test_path_with_a_space_is_fully_reduced`: it exists
because the first version of the scrubber kept the directory of a path that
contained a space ("/home/kilisan/My Docs/instrument.py" became
"My Docs/instrument.py"), and the residual check did not catch it. A redaction
that removes half a path is not a redaction.
"""

from __future__ import annotations

import json
import pathlib
import subprocess
import sys
import tempfile
import unittest

HERE = pathlib.Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import publish_report as pr  # noqa: E402

FIXTURE = HERE.parent / "src-tauri" / "tests" / "fixtures" / "payload_fail.json"


def envelope(payload: dict) -> dict:
    return {"schema": "elohim.gate/1", "run": "2026-10-02T00:00:00+00:00", "skills": [payload]}


def skill(**over) -> dict:
    base = {
        "skill": "elohim",
        "verdict": "PASS",
        "instrument": "/tmp/opencode/nc/skills/elohim/instrument/summoning_shard.py",
        "instrument_pin": {
            "status": "PASS",
            "path": "/tmp/opencode/nc/skills/elohim/instrument/summoning_shard.py",
            "expected_sha256": "a" * 64,
            "detail": "checksum and size match the ledger",
        },
        "traps": [],
        "facts": [],
        "runtime": {"total": 1.5},
        "claim_binding": {"status": "OK", "ok": True, "unverified_exemptions": 0},
        "hygiene": {"status": "OK", "ok": True, "findings": []},
    }
    base.update(over)
    return base


def bare_skill(**over) -> dict:
    """A skill with no path in it, so a redaction count of 0 is meaningful."""
    return skill(
        instrument=None,
        instrument_pin={"status": "PASS", "path": None, "expected_sha256": "a" * 64, "detail": "ok"},
        **over,
    )


class Redaction(unittest.TestCase):
    def scrub(self, payload: dict) -> tuple[dict, pr.Scrubber]:
        scrubber = pr.Scrubber()
        return scrubber.scrub(payload), scrubber

    def test_path_with_a_space_is_fully_reduced(self):
        out, s = self.scrub(
            envelope(skill(instrument="/home/kilisan/My Docs/instrument.py"))
        )
        self.assertEqual(
            out["skills"][0]["instrument"], "instrument.py", "directory survived scrubbing"
        )
        self.assertEqual(pr.residual_paths(out), [])

    def test_path_with_a_space_in_pin_path_is_fully_reduced(self):
        out, _ = self.scrub(envelope(skill()))
        out["skills"][0]["instrument_pin"]["path"] = "/home/kilisan/My Docs/instrument.py"
        out, _ = self.scrub(out)
        self.assertEqual(out["skills"][0]["instrument_pin"]["path"], "instrument.py")
        self.assertEqual(pr.residual_paths(out), [])

    def test_nested_path_value_in_a_list_is_reduced(self):
        out, _ = self.scrub(
            {"schema": "elohim.gate/1", "notes": ["/var/lib/thing/ledger.json"]}
        )
        self.assertEqual(out["notes"], ["ledger.json"])

    def test_urls_are_not_touched(self):
        out, s = self.scrub(envelope(bare_skill(detail="see https://example.com/a/b for detail")))
        self.assertIn("https://example.com/a/b", out["skills"][0]["detail"])
        self.assertEqual(s.count, 0, "a URL was counted as a redaction")

    def test_ratios_are_not_touched(self):
        out, s = self.scrub(envelope(bare_skill(verdict="4/5 skills")))
        self.assertEqual(out["skills"][0]["verdict"], "4/5 skills")
        self.assertEqual(s.count, 0, "a ratio was counted as a redaction")

    def test_schema_id_is_not_touched(self):
        out, s = self.scrub(envelope(skill()))
        self.assertEqual(out["schema"], "elohim.gate/1")
        self.assertEqual(s.count, 2, "exactly the two path fields should redact")

    def test_path_hidden_in_a_prose_field_is_still_swept(self):
        out, s = self.scrub(
            envelope(bare_skill(detail="instrument at /home/kilisan/x/i.py drifted"))
        )
        self.assertEqual(out["skills"][0]["detail"], "instrument at i.py drifted")
        self.assertEqual(pr.residual_paths(out), [])

    def test_clean_payload_redacts_nothing(self):
        out, s = self.scrub(envelope(bare_skill()))
        self.assertEqual(s.count, 0, "the counter fired on a payload with no path in it")
        self.assertEqual(pr.residual_paths(out), [])

    def test_trailing_prose_punctuation_is_kept(self):
        self.assertEqual(pr.basename("/tmp/a/b.py."), "b.py.")
        self.assertEqual(pr.basename("/tmp/a/b.py"), "b.py")


class EndToEnd(unittest.TestCase):
    def run_tool(self, payload: dict, output: pathlib.Path) -> subprocess.CompletedProcess:
        with tempfile.TemporaryDirectory() as tmp:
            source = pathlib.Path(tmp) / "in.json"
            source.write_text(json.dumps(payload))
            return subprocess.run(
                [sys.executable, str(HERE / "publish_report.py"), str(source), "-o", str(output)],
                capture_output=True,
                text=True,
            )

    def test_real_fixture_publishes_clean(self):
        if not FIXTURE.exists():
            self.skipTest(f"fixture missing: {FIXTURE}")
        with tempfile.TemporaryDirectory() as tmp:
            out = pathlib.Path(tmp) / "latest.json"
            proc = self.run_tool(json.loads(FIXTURE.read_text()), out)
            self.assertEqual(proc.returncode, 0, proc.stderr)
            doc = json.loads(out.read_text())
            self.assertGreater(doc["redacted_paths"], 0, "the counter never fired")
            self.assertEqual(pr.residual_paths(doc["payload"]), [])
            self.assertEqual(doc["payload"]["skills"][0]["instrument"], "summoning_shard.py")

    def test_wrong_schema_is_refused(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = pathlib.Path(tmp) / "latest.json"
            proc = self.run_tool({"schema": "something.else/1", "skills": []}, out)
            self.assertEqual(proc.returncode, 2)
            self.assertFalse(out.exists(), "a refused publish still wrote a file")

    def test_non_json_is_refused(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = pathlib.Path(tmp) / "latest.json"
            source = pathlib.Path(tmp) / "in.json"
            source.write_text("not json at all")
            proc = subprocess.run(
                [sys.executable, str(HERE / "publish_report.py"), str(source), "-o", str(out)],
                capture_output=True,
                text=True,
            )
            self.assertEqual(proc.returncode, 2)
            self.assertFalse(out.exists())


if __name__ == "__main__":
    unittest.main(verbosity=2)