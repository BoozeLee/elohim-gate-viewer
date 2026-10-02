# Test fixtures

Real gate payloads, captured from a scratch checkout and scrubbed. Read this
before using either file as evidence.

## These fixtures are not an oracle

A fixture records what the gate printed on one machine at one moment. It is
evidence that the viewer *renders this shape correctly*, and nothing else. It is
not evidence that the gate is correct, and a test that passes against a fixture
has not verified any fact or trap.

To re-measure the gate itself, run the real gate (`tests/test_all.py`,
`tools/check_text.py`, `tools/submit.py --check`). Those are the claims' source
of truth; this directory is only this app's input.

## What each fixture was made to be bad

- **`payload_clean.json`** — deliberately not bad. Six skills, `PASS`, 81/81
  facts verified, 38/38 traps holding, exit 0. Captured from the committed tree
  at `91691ef`. It is the shape the viewer shows on a healthy run.
- **`payload_fail.json`** — bad on purpose, and bad in **two different ways at
  once**, because a viewer tested only against one kind of failure is tested
  against half the states it will meet:
  1. `elohim` — `instrument_pin.status` is `DRIFT`. A single 1-byte edit to
     `summoning_shard.py` (30497 → 30496 bytes) was caught by the pin's size
     check. Its own 25 facts all still verified.
  2. `reproducibility` — its pin is `PASS` and yet it still failed: one of its
     facts, `sibling_instrument_pins_hold`, went `drifted` with detail
     `summary.pins_matching measured 4 against a recorded 5`. The corruption was
     found a second time, independently, by a skill that was not looking at the
     pin mechanism.

  This pairing is the reason the fixture is worth committing. A viewer that only
  learned to render "pin DRIFT" would mislabel state 2, and state 2 is the one
  that proves the cross-check works.

## Provenance and scrubbing

Produced by `/tmp/opencode/nc_fail.py`, which copies the repo to a scratch
directory, flips exactly one `verified` token to `drifted` in
`skills/elohim/instrument/summoning_shard.py`, runs
`skills/elohim-harness/scripts/harness_run.py --all --json`, and restores the
instrument byte-identical afterwards.

`build_fixtures.py` then rewrote the checkout prefix to `/srv/elohim-checkout`
and replaced child-process `stdout_tail` values with `<scrubbed>`. No key is
removed, so the full 18-key skill shape survives. No secrets were present in
either payload; that was checked, not assumed.