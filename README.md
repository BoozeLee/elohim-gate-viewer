# elohim-gate-viewer

A desktop window that runs the [elohim](../elohim) gate and shows you what it
found. One screen: a verdict, a per-skill table, and the reason next to anything
that failed.

It reads the gate's payload and draws it. It does not re-run any skill, does not
re-check any number, and does not have its own opinion about whether the tree is
healthy. If it disagrees with the gate, the gate wins and this app is wrong.

## Why it is a separate repository

elohim's own test suite runs the gate from inside its own checkout. A viewer
living in that repository could read a relative path, run the gate against
itself, and go green because it agreed with itself. Keeping the viewer outside
means the only way it can turn green is by successfully starting a gate someone
else owns.

The cost is honest and worth stating: this app does not ship a gate. It runs
whatever gate you point it at, so "the viewer works" and "the gate works" stay
separate claims.

## Requirements

- Python 3.10 or newer on `PATH`. This floor is not a guess — it is read from
  elohim's own `requires-python`. Nothing is bundled, so without Python there is
  no gate to run and the viewer says so instead of pretending.
- A gate, either on `PATH` as `elohim` or named in `ELOHIM_GATE_CMD`.

## Pointing it at a gate

```sh
# a checkout, without installing
export ELOHIM_GATE_CMD="python3 /path/to/elohim/skills/elohim-harness/scripts/harness_run.py"
npm run tauri dev
```

or with the console script installed:

```sh
pip install elohim
npm run tauri dev
```

With neither, the window opens showing exactly what is missing and how to fix
it. It does not open blank.

## Commands

| Command | What it does |
|---|---|
| `npm run tauri dev` | the app |
| `npm test` | 23 Rust tests, no gate needed, no network |
| `npm run test:gate` | one test against a real gate; needs `ELOHIM_GATE_CMD` |
| `npm run shots` | re-render the three screenshots from the committed fixtures |
| `npm run build` | typecheck and bundle the frontend |

## How it is put together

Four pieces, split so that the interesting ones need no window to test.

- `src-tauri/src/gate.rs` — finding the gate, running it, parsing the payload,
  and refusing anything untrustworthy. Pure functions over a command string; no
  Tauri types. Every failure a user can hit is a `GateError` variant whose
  message says what to do about it.
- `src-tauri/src/lib.rs` — two commands, `preflight` and `run_gate`, and one
  tagged result type. It moves values and decides nothing.
- `src/render.ts` — turns a summary into HTML. No `document`, no Tauri, so
  rendering is verifiable in a plain browser.
- `src/main.ts` — calls one command, hands the result to the renderer.

## What it refuses to do

These are the cases where showing something would be worse than showing nothing.

**An unknown payload schema.** The gate stamps `"schema": "elohim.gate/1"`. If a
future gate stamps something else, this app says which schema it saw and stops. A
viewer that renders a payload nobody pinned is the exact defect the gate exists
to catch, reproduced one layer up. `KNOWN_SCHEMAS` in `gate.rs` is the list, and
adding to it is a deliberate act.

**A payload with no `schema` key at all.** Treated as unknown, not as a default.
The key exists so a consumer can tell what shape it holds; when it is absent, it
cannot, and that is the end of the line.

**An unreadable interpreter version.** `is_new_enough` returns false for
anything it cannot parse, including an empty string. A probe that printed
something unexpected is not permission to continue.

**Empty output from the gate.** Exit 1 with a payload is a failing gate and is
displayed as one. Exit 1 with nothing printed is a broken install, and the
window says that instead — the user has to act on different things.

**A skill that was never measured.** A row for such a skill shows the reason
from `instrument_error` rather than a cheerful `0/0`. Rendering an unmeasured
skill as a clean one is the single most dangerous thing a table like this could
do.

**Absolute paths.** `skills_root`, `instrument` and `instrument_pin.path` are host
paths. They are read out and dropped; a test asserts that no host path survives
into the summary the frontend receives. What travels between machines is the
pin's sha256, and that is shown shortened to 12 characters.

## Evidence

`shots/` holds three rendered states, produced from the fixtures in
`src-tauri/tests/fixtures/` — a real passing run, a real failing run, and the
"no gate" state. `npm run shots` rebuilds them.

Those screenshots are regenerated and are the only binaries in the tree. They are
committed because a reviewer should be able to see the red state without running
anything, and `npm run shots` asserts that the pass and fail renders differ in
content rather than only in pixels.

The fixtures are captured real payloads with paths rewritten and stdout scrubbed.
They are **not** an oracle — `src-tauri/tests/fixtures/README.md` records exactly
what each one was made to break. A fixture that agrees with the viewer proves
nothing on its own; it is a regression pin, not a source of truth.

The failing payload was produced for real, not hand-written: one `verified` token
in `skills/elohim/instrument/summoning_shard.py` was flipped to `drifted`, which
changed the file by one byte. That broke two independent things — elohim's own
instrument pin went to `DRIFT`, and a `reproducibility` fact about sibling pins
went with it — and the viewer shows both, in the right rows.

## Status

Verified: the Rust gate-invocation logic (23 tests), the renderer (rendered in a
real browser and screenshotted), the IPC shapes, and the end-to-end link against
a real gate in both directions — `PASS exit=0 skills=6 passed=6 facts=81/81
traps=38/38` against the clean checkout, and `FAIL exit=1 skills=6 passed=4
failed=2 facts=80/81 traps=37/38` against the corrupted one.

Not verified: the Tauri window has never been seen. `npm run tauri dev` builds
and starts, then the process dies on `Gdk-Message: Error 71 (Protocol error
dispatching to Wayland display)`. Everything below the window is tested; the
window itself is not, and this README does not claim otherwise.
