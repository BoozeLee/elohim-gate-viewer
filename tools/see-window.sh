#!/usr/bin/env bash
# Launch the viewer, put it on the workspace that is actually showing, screenshot
# it, and shut it down -- all inside one invocation.
#
# Two facts about this box force that shape:
#
#   1. Backgrounding the app and inspecting it from a *separate* tool call does
#      not work. The harness tears down the process group when a call returns,
#      so the window is gone by the next call even though `setsid` was used. It
#      has to all happen here, in one call.
#
#   2. `grim` captures the monitor as currently composited, which on a tiling
#      compositor means the *active* workspace only. The viewer opens on a
#      workspace chosen by Hyprland, and this box has several other sessions
#      switching workspaces underneath us. So we follow the window rather than
#      the other way round: the active workspace is switched to whichever
#      workspace the viewer is on, and switched back when the script is done.
#
#      That direction is not a stylistic choice. Moving the *window* to the
#      active workspace was tried first and never converged -- ten attempts left
#      the window where it was and the loop reported failure while printing that
#      same window, alive and mapped, in the error's own diagnostic dump. Asking
#      the window to move fights the compositor; asking the workspace to follow
#      does not.
#
# Hyperland 0.56 dispatch syntax, learned here by probing the Lua API: the old
# `hyprctl dispatch workspace 4` form is dead. What works is
#   hyprctl dispatch 'hl.dsp.global("workspace 4")'
# and for moving a window between workspaces:
#   hyprctl dispatch 'hl.dsp.workspace.move({ window = "0x...", workspace = N })'
# `hyprctl eval` swallows return values, but an `error(msg)` inside it prints
# msg, which is how the available field names were enumerated.
set -uo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
scratch="${TMPDIR:-/tmp}/elohim-gate-viewer-capture"
mkdir -p "$scratch"
out="${1:-$scratch/gate-window.png}"
log="$scratch/see-window.log"

cd "$root"

# Remembered so the workspace can be put back. This box has other sessions
# working, and leaving them on the wrong workspace is a real cost to them --
# one this script caused, so one it undoes.
start_ws=$(hyprctl activeworkspace -j 2>/dev/null | python3 -c 'import sys,json; print(json.load(sys.stdin)["id"])' 2>/dev/null || echo "")
restore_workspace() {
    [ -n "$start_ws" ] || return 0
    current=$(hyprctl activeworkspace -j 2>/dev/null | python3 -c 'import sys,json; print(json.load(sys.stdin)["id"])' 2>/dev/null || echo "")
    [ -n "$current" ] && [ "$current" != "$start_ws" ] || return 0
    hyprctl dispatch "hl.dsp.global(\"workspace $start_ws\")" >/dev/null 2>&1
}
trap restore_workspace EXIT

export LIBGL_ALWAYS_SOFTWARE=1
export GSK_RENDERER=cairo
export WEBKIT_DISABLE_COMPOSITING_MODE=1

# Something else on this machine sends the window a CloseRequested a few seconds
# after it maps -- with no matching request anywhere in this app's code, and the
# process leaving with code 0 rather than dying on a signal. Measured: suppressing
# that close keeps the window up indefinitely, which is why the app honours
# ELOHIM_VIEWER_HOLD_OPEN. It is a workaround for the environment, not for the
# app, so it is opt-in and off by default.
export ELOHIM_VIEWER_HOLD_OPEN=1

# ELOHIM_VIEWER_RUN asks the app to run the gate on load instead of waiting for a
# button press, and asks this script to wait for that run. Both halves are needed
# and they are wired here, before the launcher is spawned: VITE_AUTORUN is read by
# vite at *build* time and only exists if it is in the environment when vite is
# started. Setting only ELOHIM_VIEWER_RUN produced a window that loaded, logged
# `ui: idle`, and never ran the gate -- a harness waiting for an event nothing had
# started.
#
# Deliberate: pressing the button needs a keypress or a click, and on a shared box
# both go to whichever window holds focus, which is not reliably ours. Sending
# synthetic input at another agent's terminal to take a screenshot is not a trade
# worth making.
if [ "${ELOHIM_VIEWER_RUN:-0}" = "1" ]; then
    export VITE_AUTORUN=1
fi

rm -f "$log"
setsid npm run tauri dev </dev/null >"$log" 2>&1 &
launcher=$!

# Wait for the window to be mapped, not merely for the process to exist: an
# earlier run reported a window that was about to die, and a screenshot taken
# then proves nothing.
addr=""
for _ in $(seq 1 60); do
    sleep 1
    addr=$(hyprctl clients -j 2>/dev/null | python3 -c '
import sys, json
try:
    clients = json.load(sys.stdin)
except Exception:
    raise SystemExit(1)
for c in clients:
    if "lohim" in c["class"].lower() and c.get("mapped"):
        print(c["address"]); break
' 2>/dev/null) && [ -n "$addr" ] && break
    addr=""
done

if [ -z "$addr" ]; then
    echo "FAILED: no mapped elohim window after 60s"
    echo "--- launcher log:"; tail -20 "$log"
    kill "$launcher" 2>/dev/null
    pkill -f "[e]lohim-gate-viewer" 2>/dev/null
    exit 1
fi

echo "window at $addr"

# Let it settle: the payload is fetched by the frontend over IPC, and the first
# frame is drawn before the table exists. Screenshotting too early yields a
# correct-looking window showing nothing, which is exactly the kind of evidence
# that lies.
sleep 6

# Confirm it is still alive after that settle time. This is the whole point of
# the exercise: a window that mapped and then vanished is not a working window.
if ! hyprctl clients -j | python3 -c "
import sys, json
clients = json.load(sys.stdin)
sys.exit(0 if any('$addr' == c['address'] for c in clients) else 1)
"; then
    echo "FAILED: window $addr did not survive the settle period"
    tail -20 "$log"
    pkill -f "[e]lohim-gate-viewer" 2>/dev/null
    kill "$launcher" 2>/dev/null
    exit 1
fi
echo "window survived the settle period"

full="$scratch/gate-window-full.png"

# Geometry is read fresh by raise_viewer before each capture; nothing here needs
# a rect of its own.

# Capture the viewer's own rectangle out of the composited monitor. A function so
# it can be called more than once when the window changes state mid-capture.
# Capture the viewer's own rectangle, and *prove the crop is the viewer* before
# accepting it.
#
# A screenshot of something else is worse than no screenshot. An earlier run
# produced a crop of another session's terminal showing an unrelated dialog,
# entirely plausible as a picture of this app -- geometry, mapped and workspace
# were all correct at the moment they were checked, and another window covered
# ours between that check and grim. So the crop is now measured rather than
# trusted.
#
# The discriminator is the viewer's verdict card. Measured over a correct crop
# and a wrong-terminal crop: the correct one holds thousands of strongly
# saturated accent pixels (the PASS border and pill, ~2209 px of
# (119,207,150)); the wrong one peaks at 15 px of any single colour. A threshold
# far from that gap is deliberate -- the two distributions are three orders of
# magnitude apart, so the exact number does not matter.
capture() {
    local dest="$1"
    local attempt
    for attempt in 1 2 3 4 5; do
        raise_viewer || return 1
        grim "$full" || { echo "grim failed" >&2; return 1; }
        if python3 - "$full" "$dest" "$X" "$Y" "$W" "$H" <<'PY'
import sys
from PIL import Image

full_path, out_path, x, y, w, h = sys.argv[1], sys.argv[2], *map(int, sys.argv[3:7])
image = Image.open(full_path).convert("RGB")
crop = image.crop((x, y, x + w, y + h))

# Count strongly saturated accent pixels before upscaling, so the count is in the
# window's own pixels rather than the screenshot's.
accent = 0
for r, g, b in crop.getdata():
    if max(r, g, b) - min(r, g, b) > 60 and max(r, g, b) > 110:
        accent += 1
if accent < 300:
    print(f"NOT THE VIEWER: {accent} accent pixels (need 300+); crop is something else")
    raise SystemExit(1)

scale = 2 if w < 1100 else 1
if scale > 1:
    crop = crop.resize((crop.width * scale, crop.height * scale), Image.LANCZOS)
crop.save(out_path)
print(f"VERIFIED ({accent} accent px): ({x}, {y}, {w}, {h}) -> {out_path} at {crop.width}x{crop.height}")
PY
        then
            return 0
        fi
        echo "attempt $attempt captured something else; re-raising and retrying" >&2
        sleep 2
    done
    echo "FAIL: five consecutive captures were not the viewer. Not writing $dest." >&2
    return 1
}

# Read the viewer's geometry right now, from the address we captured at startup.
read_rect() {
    hyprctl clients -j | python3 -c "
import sys, json
for c in json.load(sys.stdin):
    if c['address'] == '$addr':
        x, y = c['at']; w, h = c['size']
        print(f'{x} {y} {w} {h}'); break
"
}

# Put the viewer back on top, right now, and confirm it is genuinely the
# topmost client before anything is captured.
#
# Three things on this box fight for the foreground: other sessions switch
# workspaces, something closes windows that it did not open, and grim only
# captures the active workspace. Focusing once at startup and grimming thirty
# seconds later captures whatever ended up in front instead -- which is how an
# earlier run produced a screenshot of somebody else's terminal that looked
# entirely plausible. So the check is repeated immediately before every capture,
# and the window is pinned so workspace-switching automation leaves it alone.
# The address of the mapped viewer window, re-resolved on every call.
#
# Resolved once and cached, the loop below compared against a stale value when
# Hyprland handed the window a new address: the state query matched nothing, so
# all ten attempts "failed" while the window was plainly alive in the very dump
# the failure message prints. Anything that must find this window has to be able
# to find it again.
find_addr() {
    hyprctl clients -j 2>/dev/null | python3 -c '
import sys, json
try:
    clients = json.load(sys.stdin)
except Exception:
    raise SystemExit(1)
for c in clients:
    if "lohim" in c["class"].lower() and c.get("mapped"):
        print(c["address"]); break
'
}

raise_viewer() {
    for _ in 1 2 3 4 5 6 7 8 9 10; do
        addr=$(find_addr)
        if [ -z "$addr" ]; then
            sleep 1
            continue
        fi
        active=$(hyprctl activeworkspace -j | python3 -c 'import sys,json; print(json.load(sys.stdin)["id"])')

        # Force a size the table fits in. The window has been observed at
        # 941x508 and at 466x249 across runs despite tauri.conf.json asking for
        # 800x600, and at 466x249 the per-skill rows -- including the DRIFT detail
        # that is the whole point of the failing state -- fall below the fold. A
        # screenshot of the verdict alone would understate what the app shows.
        # Skipped once the window is already at least this big, so a run that
        # opens at a good size is not disturbed.
        cur=$(hyprctl clients -j 2>/dev/null | python3 -c "
import sys, json
for c in json.load(sys.stdin):
    if c['address'] == '$addr':
        print(c['size'][0], c['size'][1]); break
" 2>/dev/null)
        read -r CW CH <<<"${cur:-0 0}"
        # 900, not 1000. The window has been seen at 941x508 and at 466x508 across
        # runs despite tauri.conf.json asking for 800x600, and 941 is wide enough
        # for the table. A 1000px floor flagged the good width as too small and
        # fought a window that was already correct -- which is how this ended up
        # fullscreening a healthy window and losing it. A resize is also only
        # ever attempted for the genuinely narrow case, where it has been
        # observed to work.
        if [ "${CW:-0}" -lt "${ELOHIM_VIEWER_MIN_W:-900}" ] 2>/dev/null; then
            hyprctl dispatch "hl.dsp.window.resize({ window = \"$addr\", width = ${ELOHIM_VIEWER_MIN_W:-900}, height = ${ELOHIM_VIEWER_MIN_H:-900} })" >/dev/null 2>&1
            sleep 1
        fi
        # Success is "the viewer is on the workspace grim will capture", not "the
        # viewer holds focus". Requiring focus made this fail for the wrong
        # reason: other sessions on this box steal focus constantly, and focus
        # says nothing about whether the window is visible in the capture.
        state=$(python3 -c "
import json, subprocess
clients = json.loads(subprocess.run(['hyprctl','clients','-j'], capture_output=True, text=True).stdout)
active = json.loads(subprocess.run(['hyprctl','activeworkspace','-j'], capture_output=True, text=True).stdout)['id']
for c in clients:
    if c['address'] == '$addr':
        print(c['workspace']['id'], int(c['mapped']), c['at'][0], c['at'][1], c['size'][0], c['size'][1])
        break
")
        ws_now=$(echo "$state" | cut -d' ' -f1)
        mapped=$(echo "$state" | cut -d' ' -f2)
        if [ "$ws_now" = "$active" ] && [ "$mapped" = "1" ]; then
            read -r X Y W H <<<"$(echo "$state" | cut -d' ' -f3-6)"
            echo "viewer mapped on active workspace $active at (${X},${Y}) ${W}x${H}"
            return 0
        fi

        # Switch the active workspace to the one the viewer is actually on, rather
        # than moving the window to the active workspace. Moving the window was
        # the first thing tried and it never converged: across ten attempts the
        # window stayed put and the loop reported failure while printing the
        # window, alive and mapped, in the very dump below the error. Following
        # the window with the workspace converges in one step because it does not
        # ask the window to go anywhere. It briefly disturbs other sessions,
        # which is why the script restores the workspace it started on.
        if [ "$mapped" != "1" ] || [ -z "$ws_now" ]; then
            sleep 1
            continue
        fi
        if [ "$ws_now" != "$active" ]; then
            hyprctl dispatch "hl.dsp.global(\"workspace $ws_now\")" >/dev/null 2>&1
            sleep 1
            continue
        fi
        # Same workspace as what grim will capture. Lift it above whatever else
        # shares that workspace -- the crop is still verified by accent pixels
        # afterwards, so this is about odds, not about correctness.
        hyprctl dispatch "hl.dsp.focus({ window = \"$addr\" })" >/dev/null 2>&1
        read -r X Y W H <<<"$(echo "$state" | cut -d' ' -f3-6)"
        echo "viewer mapped on active workspace $active at (${X},${Y}) ${W}x${H}"
        return 0
    done

    echo "FAIL: viewer is not mapped on the active workspace, or is gone." >&2
    echo "--- active workspace now: $(hyprctl activeworkspace -j 2>/dev/null | python3 -c 'import sys,json; print(json.load(sys.stdin)["id"])' 2>/dev/null)" >&2
    echo "--- elohim clients now:" >&2
    hyprctl clients -j 2>/dev/null | python3 -c "
import sys, json
for c in json.load(sys.stdin):
    if 'lohim' in c['class'].lower():
        print(' ', c['address'], c['at'], c['size'], 'ws=', c['workspace']['id'], 'special=', c['workspace'].get('name'), 'mapped=', c['mapped'])
" >&2
    echo "--- app alive?" >&2
    pgrep -af "[e]lohim-gate-viewer" >&2 || echo "  no: the app exited" >&2
    return 1
}

# With ELOHIM_VIEWER_RUN=1 the app starts the gate itself on load (see
# VITE_AUTORUN in src/main.ts). That is deliberate: pressing the button needs
# either a keypress or a click, and on a shared box both go to whichever window
# happens to hold focus -- which, while other sessions are working, is not
# reliably ours. Sending synthetic input at another agent's terminal to take a
# screenshot is not a trade worth making.
if [ "${ELOHIM_VIEWER_RUN:-0}" = "1" ]; then
    # Wait for the gate to actually finish, rather than sleeping a guessed
    # interval. A fixed sleep is wrong in both directions here: too short
    # screenshots a spinner, and the gate's runtime on a loaded machine has
    # ranged from 15s to well over 45s. The app writes one `gate-run:` line to
    # stderr when the result is in, so the log is the signal.
    #
    # `ELOHIM_VIEWER_TIMEOUT` is a ceiling, not a wait: if the gate is slower than
    # this the run fails loudly instead of quietly screenshotting the in-progress
    # hint, which is what a plain sleep would do.
    deadline="${ELOHIM_VIEWER_TIMEOUT:-180}"
    waited=0
    # Wait for the UI to report a SETTLED state, not for the gate to finish.
    #
    # `gate-run:` means the backend has a result; `ui: PASS 6/6` means the window
    # has painted it. Those are different moments and the gap between them is
    # what broke this: main.ts re-runs the gate (2-3 times per app lifetime,
    # unexplained), so a capture taken after `gate-run:` regularly landed on a
    # fresh run's in-progress hint -- which has no verdict card, and so measured
    # zero accent pixels and was correctly rejected five times as "not the
    # viewer" while the window was mapped and correct the entire time.
    settled='^ui: (PASS|FAIL|unavailable|failed)'
    while [ "$waited" -lt "$deadline" ]; do
        if grep -qE "$settled" "$log" 2>/dev/null; then
            break
        fi
        if ! pgrep -f "[e]lohim-gate-viewer" >/dev/null; then
            echo "FAIL: the app exited before the gate finished" >&2
            tail -20 "$log" >&2
            kill "$launcher" 2>/dev/null
            exit 1
        fi
        sleep 5
        waited=$((waited + 5))
    done

    if ! grep -qE "$settled" "$log" 2>/dev/null; then
        echo "FAIL: no settled UI state after ${deadline}s" >&2
        tail -20 "$log" >&2
        kill "$launcher" 2>/dev/null
        pkill -f "[e]lohim-gate-viewer" 2>/dev/null
        exit 1
    fi

    # Let the last frame land. The result is returned to the webview and painted
    # asynchronously; capturing the instant the log line appears can catch the
    # window before the repaint.
    sleep 4
    echo "gate finished after ~${waited}s:"
    grep "^gate-run:" "$log"
    grep -E "$settled" "$log" | tail -1
    capture "${out%.png}-soon.png" \
        || { kill "$launcher" 2>/dev/null; pkill -f "[e]lohim-gate-viewer" 2>/dev/null; exit 1; }

    # A second capture well after the fact. The gate finishing in the backend is
    # not the same claim as the window showing it, and the difference has to be
    # visible rather than assumed.
    sleep "${ELOHIM_VIEWER_LATE:-45}"
    capture "${out%.png}-late.png" \
        || { kill "$launcher" 2>/dev/null; pkill -f "[e]lohim-gate-viewer" 2>/dev/null; exit 1; }
fi

capture "$out" \
    || { kill "$launcher" 2>/dev/null; pkill -f "[e]lohim-gate-viewer" 2>/dev/null; exit 1; }

kill "$launcher" 2>/dev/null
pkill -f "[e]lohim-gate-viewer" 2>/dev/null
pkill -f "[t]auri dev" 2>/dev/null
echo "done"