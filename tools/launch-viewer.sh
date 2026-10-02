#!/usr/bin/env bash
# Launch the viewer detached so a shell wrapper never holds a pipe open on it.
# Two env vars matter, both forced here rather than left to the caller:
#
# The one thing that matters here is software rendering. WebKit's accelerated
# path asks for a GBM buffer that this GPU will not give it:
#
#   Failed to create GBM buffer of size 1600x1200: Invalid argument
#
# That failure is what actually kills the window. "Gdk-Message: Error 71
# (Protocol error) dispatching to Wayland display" is a downstream symptom of
# it, not a Wayland incompatibility -- an earlier version of this script read it
# as one and forced X11, which was wrong twice over:
#
#   * native Wayland works fine once rendering is software (verified: window
#     mapped, xwayland: false, alive past 12s)
#   * under GDK_BACKEND=x11 the app instead exited by itself, code 0, ~4s in,
#     with no panic and no output. gdb reported "[Inferior 1 (pid) exited
#     normally]"; a run-event trace ended after MainEventsCleared with no
#     Destroyed, ExitRequested or Exit. That is the signature of an Xlib XIOError
#     (default handler calls exit(0)), so forcing X11 traded one dead window for
#     a quieter dead window.
#
# So: no GDK_BACKEND. If someone adds one back, read the above first.
#
# Set ELOHIM_GATE_CMD to point at a gate; otherwise the app resolves `elohim`
# from PATH and will say so plainly if it is not there.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

log="${1:-${TMPDIR:-/tmp}/elohim-gate-viewer.log}"
rm -f "$log"

export LIBGL_ALWAYS_SOFTWARE=1
export GSK_RENDERER=cairo
export WEBKIT_DISABLE_COMPOSITING_MODE=1
export RUST_LOG=warn

# `npm run tauri dev` drives vite itself, so the frontend is served and the
# window opens against the dev URL in one step.
exec setsid nohup npm run tauri dev </dev/null >"$log" 2>&1 &
echo "launched; log=$log"
