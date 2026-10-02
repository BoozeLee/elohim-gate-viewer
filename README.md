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

The command that produced a run is printed as provenance above the table, and it
is the one place a path could reach the screen. It is shown as bare filenames —
`harness_run.py --all --json`, not the checkout it came from.

That is not timidity, and it reverses an earlier decision here. `window-pass.png`
used to carry `/home/<user>/…` into a committed PNG, because this line printed
the command verbatim and a screenshot cannot be un-published by editing the text
around it. Knowing *which command* ran is the provenance worth having. Knowing
whose machine it ran on is the half that leaks, and it was never the half that
made the verdict legible.

## Evidence

`shots/` holds five renders.

Three are the renderer alone, from the fixtures in `src-tauri/tests/fixtures/` —
a passing run, a failing run, and the "no gate" state. `npm run shots` rebuilds
them, and asserts that the pass and fail renders differ in content rather than
only in pixels.

Two are the real application window on a real compositor, captured off a live
screen. The gate they display is fed from the committed fixtures by
`tools/emit_fixture.py`, not from a live gate run:

| | |
|---|---|
| `window-pass.png` | `payload_clean.json`: `PASS — 6 of 6 skills verified`, 81/81 facts, 38/38 traps, exit 0 |
| `window-fail.png` | `payload_fail.json`: `FAIL — 4 of 6 skills verified, 2 failed`, 80/81 facts with `1 drifted`, exit 1 |

That indirection is what makes them reproducible. Taken against a live gate,
`window-pass.png` required the gate to pass on the day it was captured, and it
does not pass today.

The window ones are committed because they are the only evidence that the
rendered pixels are what the tests think they are. Everything below the window
was verified by `cargo test`; a passing test suite says nothing about what a
user sees. `tools/see-window.sh` produced them, and it does not take a picture
unless the crop it took is provably the window — see below.

One blemish is in `window-fail.png` and is not the app: the circled `H` at the top
right is Hyprland's workspace indicator, drawn because the capture harness
switches workspaces to follow the window. It is in no other screenshot and it
covers no field. It is left in rather than cropped, because a crop that removes
it also removes the pixels that prove the shot is the whole window.

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

To reproduce the committed shots, point the gate at a fixture instead:

```sh
ELOHIM_VIEWER_RUN=1 \
ELOHIM_GATE_CMD="python3 $PWD/tools/emit_fixture.py $PWD/src-tauri/tests/fixtures/payload_fail.json" \
./tools/see-window.sh window-fail.png
```

Use absolute paths there. The app spawns the gate with its own working
directory, so a relative path to the script is not found and the window renders
"unavailable" instead of the fixture.

`emit_fixture.py` exists because the app appends `--all --json` to whatever
`ELOHIM_GATE_CMD` names, so `cat payload_fail.json` exits 1 having printed
nothing and the window reports "no payload". It accepts and ignores those flags,
validates the schema before printing, and mirrors the harness on the exit code —
a payload with a failing skill exits 1, exactly as a real failing run does.

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

Verified end to end, in the real window, in both directions:

- `window-pass.png` — `payload_clean.json`: `PASS`, 6/6 skills, 81/81 facts,
  38/38 traps, exit 0.
- `window-fail.png` — `payload_fail.json`: `FAIL`, 4/6 skills, 80/81 facts with
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

## Known dependency advisory

Dependabot reports one: [GHSA-wrw7-89jp-8q8g](https://github.com/advisories/GHSA-wrw7-89jp-8q8g),
moderate, unsound `Iterator`/`DoubleEndedIterator` impls for
`glib::VariantStrIter`, fixed in `glib` 0.20.0. This build resolves `glib`
0.18.5.

It is not fixable within Tauri 2. `cargo update -p glib` reports zero packages
available: `glib` 0.18 is what the whole GTK stack of the current Tauri ships
(`gtk` 0.18.2 via `tauri` 2.12.1), and the first release carrying `glib` 0.20 is
`tauri` 3.0.0-alpha.4. Moving to an alpha framework to clear a moderate advisory
in a transitive binding is the wrong trade, so it is left open and stated here.

Exposure for this app is small but not zero, so the reasoning is written down
rather than asserted away: the unsound code is a Rust-side iterator over GLib
variants, and nothing in this app references `glib` or any `Variant` type. The
dependency exists only inside Tauri's GTK webview plumbing.
