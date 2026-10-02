import { invoke } from "@tauri-apps/api/core";
import {
  renderProvenance,
  renderSummary,
  renderUnavailable,
  type GateResponse,
} from "./render";

const panel = document.querySelector<HTMLElement>("#panel")!;
const runButton = document.querySelector<HTMLButtonElement>("#run")!;
const provenance = document.querySelector<HTMLElement>("#provenance")!;

// Tell the backend what is on screen. The backend cannot see the window, and a
// screenshot is a thing you take once -- not something you can poll while
// working out why a result reached the Rust side but never appeared here. This
// line in the log is the difference between "the gate finished" and "the gate
// finished and the window is showing it".
//
// Fire-and-forget on purpose: reporting must never be able to stall or fail the
// render it is reporting about.
function report(state: string): void {
  void invoke("report", { state }).catch(() => {});
}

// The window title is a second, independent observable: the compositor knows it
// without this app telling it anything, so it can confirm what the webview is
// showing even if IPC reporting were the thing that is broken.
function setTitle(state: string): void {
  document.title = `elohim gate — ${state}`;
}

function show(html: string, provenanceHtml = "", state = "unknown"): void {
  panel.innerHTML = html;
  provenance.innerHTML = provenanceHtml;
  setTitle(state);
  report(state);
}

async function run(): Promise<void> {
  runButton.disabled = true;
  show(`<p class="hint">Running the gate. This takes a few seconds.</p>`, "", "running");
  try {
    const response = await invoke<GateResponse>("run_gate");
    if (response.status === "ready") {
      const summary = response.summary;
      show(
        renderSummary(summary),
        renderProvenance(summary),
        `${summary.verdict} ${summary.passed}/${summary.skills}`,
      );
    } else {
      show(renderUnavailable(response.message), "", "unavailable");
    }
  } catch (error) {
    show(renderUnavailable(String(error)), "", "failed");
  } finally {
    runButton.disabled = false;
  }
}

runButton.addEventListener("click", () => void run());

if (import.meta.env.VITE_AUTORUN === "1") {
  // Runs the gate as soon as the window opens. For unattended captures and
  // screenshots, where nobody is present to click the button. Opt-in, so the
  // normal case stays "nothing runs until you ask", which matters because the
  // gate costs real CPU.
  void run();
} else {
  show(`<p class="hint">Not run yet. Press <kbd>Run gate</kbd>.</p>`, "", "idle");
}