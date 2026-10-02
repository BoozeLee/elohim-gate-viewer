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

**Absolute paths, per skill.** `skills_root`, `instrument` and
`instrument_pin.path` are host paths specific to whichever machine ran the gate.
They are read out and dropped; a test asserts that no host path reaches the
frontend. What travels between machines is the pin's sha256, shown shortened to
12 characters.

The one host path on screen is the invocation command itself, printed as
provenance above the table. That is deliberate and it is the exception, not an
oversight: a verdict with no idea which checkout produced it is the failure this
app exists to make visible. Both window screenshots show it.

## Evidence

`shots/` holds five renders.

Three are the renderer alone, from the fixtures in `src-tauri/tests/fixtures/` —
a passing run, a failing run, and the "no gate" state. `npm run shots` rebuilds
them, and asserts that the pass and fail renders differ in content rather than
only in pixels.

Two are the real application window, captured off a live screen:

| | |
|---|---|
| `window-pass.png` | the window against a clean checkout: `PASS — 6 of 6 skills verified`, 81/81 facts, 38/38 traps, exit 0 |
| `window-fail.png` | the window against a deliberately corrupted tree: `FAIL — 4 of 6 skills verified, 2 failed`, 80/81 facts with `1 drifted`, exit 1 |

The window ones are committed because they are the only evidence that the
rendered pixels are what the tests think they are. Everything below the window
was verified by `cargo test`; a passing test suite says nothing about what a
user sees. `tools/see-window.sh` produced them, and it does not take a picture
unless the crop it took is provably the window — see below.

The failing payload was produced for real, not hand-written: one `verified` token
in `skills/elohim/instrument/summoning_shard.py` was flipped to `drifted`, which
changed the file by one byte. That broke two independent things — elohim's own
instrument pin went to `DRIFT`, and a `reproducibility` fact about sibling pins
went with it. `window-fail.png` shows both, in the right rows, and shows them
differing: `elohim` is `FAIL / DRIFT` while `reproducibility` is `FAIL / PASS`,
a green pin under a red verdict.

The fixtures are captured real payloads with paths rewritten and stdout scrubbed.
They are **not** an oracle — `src-tauri/tests/fixtures/README.md` records exactly
what each one was made to break. A fixture that agrees with the viewer proves
nothing on its own; it is a regression pin, not a source of truth.

## Running it on a machine with no GPU

WebKit allocates a GL render surface on startup and fails with `Failed to create
GBM buffer of size ... Invalid argument` on a box without a usable GPU. That
failure is reported as `Gdk-Message: Error 71 (Protocol error) dispatching to
Wayland display`, which reads like a compositor incompatibility and is not one —
Wayland is fine. Force software rendering and the error goes away:

```sh
LIBGL_ALWAYS_SOFTWARE=1 GSK_RENDERER=cairo WEBKIT_DISABLE_COMPOSITING_MODE=1 npm run tauri dev
```

`tools/launch-viewer.sh` does this for you.

Do not reach for `GDK_BACKEND=x11` as a workaround. It is not a fix: it makes the
app exit silently with status 0 after about four seconds, with no panic and no
output. Measured, twice.

## Screenshotting the window

`tools/see-window.sh` launches the app, waits for the gate, moves the window into
view, and captures it.

```sh
ELOHIM_VIEWER_RUN=1 ELOHIM_GATE_CMD="python3 /path/to/harness_run.py" ./tools/see-window.sh out.png
```

It does not take a picture unless the crop is provably the window. A crop is
accepted only if it contains at least 300 strongly saturated pixels — the
verdict card's border and pill. A crop of some other window on the same
workspace can match the geometry, be mapped, and still be the wrong picture;
those two checks together did not stop it, and the pixel count did. On the run
that produced `window-fail.png` this rejected four wrong crops in a row before
accepting a real one.

Two opt-in environment variables exist for that harness:

- `VITE_AUTORUN=1` (set for you when `ELOHIM_VIEWER_RUN=1`) makes the window run
  the gate on load instead of waiting for the button.
- `ELOHIM_VIEWER_HOLD_OPEN=1` makes the app refuse to close. See below.

Known behaviour on a shared desktop: with `VITE_AUTORUN=1` the gate can run
again a minute or two into a capture, with its own runtime, and not because this
app asked. Measured with an instrumented build that logged a random id per page
load, the document's `click` events, and the navigation type:

```
PROBE module-eval id=dx0l6i navType=navigate at=76ms
PROBE run-enter  id=dx0l6i at=77ms          <- VITE_AUTORUN, once
PROBE click      isTrusted=true detail=1 at=26086ms target=BUTTON#run active=run
PROBE run-enter  id=dx0l6i at=26086ms       <- the button, clicked
```

One page load, so not a reload and not a second autorun. `isTrusted=true` means
a real input event rather than a scripted `el.click()`; `detail=1` means a single
pointer click, where keyboard activation of a button reports `detail=0`. The
button held focus because `raise_viewer` focuses the window on every capture
attempt, so something else on this desktop — another agent working in a terminal
— clicked at the coordinates the Run button was sitting at.

The app is behaving correctly: a clicked button re-runs the gate. The capture
harness tolerates it, because a restarted run puts the window back into its
in-progress state, which fails the accent-pixel check and is retried. `VITE_AUTORUN`
is off in normal use, so none of this is reachable without the harness.

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

Verified end to end, in the real window, against real gates in both directions:

- `window-pass.png` — clean checkout: `PASS`, 6/6 skills, 81/81 facts, 38/38
  traps, exit 0.
- `window-fail.png` — corrupted tree: `FAIL`, 4/6 skills, 80/81 facts with
  `1 drifted`, 37/38 traps, exit 1, both independent failures visible in their
  rows.
- 23 Rust tests, the renderer in a real browser, and the same round trip driven
  headlessly via `npm run test:gate`.

Two bugs were found by doing this that no test found:

**The gate ran on the UI thread.** `run_gate` was a plain `#[tauri::command]`,
which runs on the main thread — the same thread that drives the event loop — so
a fifteen-second gate froze the entire window. It is now `async` and delegates
to `spawn_blocking`.

**The real-gate test was a false pass.** It returned early when
`ELOHIM_GATE_CMD` was unset, and cargo counted that as a pass, so the one test
covering the end-to-end link printed green while executing nothing. It is now
`#[ignore]`d with a reason, so cargo reports it as ignored, and runs via
`npm run test:gate`.

Not verified: nothing about the rendering above is claimed on the strength of a
passing test. The screenshots are the evidence, and they were taken from a live
window on a live compositor.

One environment caveat, not a defect in this app: on the machine these were
captured on, something outside the app sends the window a close request a few
seconds after it maps. The trace is `CloseRequested` → `Destroyed` →
`ExitRequested`, with no matching request anywhere in this code, and suppressing
it with `api.prevent_close()` makes the window survive indefinitely.
`ELOHIM_VIEWER_HOLD_OPEN=1` enables that suppression for the screenshot harness.
It is off by default, and should stay that way: on a normal desktop it would
make the window ignore a close the user actually asked for.
