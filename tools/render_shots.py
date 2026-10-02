#!/usr/bin/env python3
"""Screenshot the real renderer against real gate payloads.

Builds each summary with `cargo run --example dump_summary`, which calls the
same `summarize` the Tauri command calls, so these images show what the app
shows. Nothing here is hand-written HTML.

    python3 tools/render_shots.py

Writes PNGs into shots/ and prints each path. Needs the vite dev server for
module loading; this script starts and stops it itself.
"""

import hashlib
import json
import pathlib
import shutil
import subprocess
import sys
import time

ROOT = pathlib.Path(__file__).resolve().parent.parent
SHOTS = ROOT / "shots"
PORT = 1430

# (output stem, fixture, exit code, served filename, query, text that must appear)
CASES = [
    (
        "pass", "payload_clean.json", 0, "summary_clean.json",
        "summary=summary_clean.json", "PASS",
    ),
    (
        "fail", "payload_fail.json", 1, "summary_fail.json",
        "summary=summary_fail.json", "2 failed",
    ),
    (
        "unavailable", None, None, None,
        "mode=unavailable", "No gate result to show",
    ),
]

CHROMIUM = shutil.which("chromium") or shutil.which("chromium-browser") or shutil.which("google-chrome")


def build_summaries() -> None:
    SHOTS.mkdir(exist_ok=True)
    for output, fixture, exit_code, served_name, _query, _expected in CASES:
        if fixture is None:
            continue
        result = subprocess.run(
            [
                "cargo", "run", "--quiet", "--example", "dump_summary",
                "--", fixture, str(exit_code),
            ],
            cwd=ROOT / "src-tauri",
            capture_output=True,
            text=True,
        )
        if result.returncode != 0:
            sys.exit(f"dump_summary failed for {fixture}:\n{result.stderr}")
        # Round-tripped so a field the frontend does not declare is caught here
        # rather than rendering as `undefined` in the picture.
        summary = json.loads(result.stdout)
        (SHOTS / served_name).write_text(json.dumps(summary))
        print(f"{served_name}: {summary['verdict']} exit={summary['exit_code']}")


def shoot(server_url: str) -> list[pathlib.Path]:
    """Render each case and require the expected text to actually be on screen.

    Playwright rather than `chromium --screenshot` because the harness fetches
    its summary after load. The one-shot screenshot fires on load and captured
    the literal word "loading…" for the first case -- a plausible-looking image
    of a panel with no verdict in it. Waiting for the readiness flag, and then
    asserting the expected text is present, is what turns a picture into
    evidence.
    """
    from playwright.sync_api import sync_playwright

    written = []
    with sync_playwright() as p:
        browser = p.chromium.launch(args=["--no-sandbox"])
        page = browser.new_page(viewport={"width": 1100, "height": 760}, device_scale_factor=2)
        for output, _fixture, _exit, _served, query, expected in CASES:
            page.goto(f"{server_url}/render-harness.html?{query}", wait_until="load")
            page.wait_for_selector("body[data-ready='true']", timeout=15_000)
            body = page.inner_text("#panel")
            if expected not in body:
                sys.exit(
                    f"{output}: expected {expected!r} in the rendered panel, got:\n{body[:400]}"
                )
            target = SHOTS / f"{output}.png"
            page.screenshot(path=str(target), full_page=True)
            written.append(target)
            print(f"{target} ({target.stat().st_size} bytes) shows {expected!r}")
        browser.close()
    return written


def main() -> int:
    build_summaries()

    server = subprocess.Popen(
        ["npx", "vite", "--port", str(PORT), "--strictPort"],
        cwd=ROOT, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
    )
    try:
        # Vite prints its URL before it is actually accepting requests.
        for _ in range(60):
            probe = subprocess.run(
                ["curl", "-sf", "-o", "/dev/null", f"http://localhost:{PORT}/render-harness.html"],
                capture_output=True,
            )
            if probe.returncode == 0:
                break
            time.sleep(0.5)
        else:
            sys.exit("vite did not come up")

        written = shoot(f"http://localhost:{PORT}")

        # The harness's own negative control. A first run of this script produced
        # two byte-identical PNGs because the query string was built as
        # `?summary_fail.json` -- no key -- so both cases silently fell back to
        # the harness default and every screenshot showed the clean run. Nothing
        # errored. A screenshot set that cannot tell its own states apart is not
        # evidence of anything, so the difference is required.
        digests = {path.stem: hashlib.sha256(path.read_bytes()).hexdigest() for path in written}
        if digests.get("pass") == digests.get("fail"):
            sys.exit(
                "pass.png and fail.png are identical -- the harness rendered one "
                "state for both. This is the harness lying, not a real result."
            )
        return 0
    finally:
        server.terminate()
        server.wait(timeout=10)


if __name__ == "__main__":
    raise SystemExit(main())