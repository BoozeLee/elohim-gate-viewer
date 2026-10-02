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

function show(html: string, provenanceHtml = ""): void {
  panel.innerHTML = html;
  provenance.innerHTML = provenanceHtml;
}

async function run(): Promise<void> {
  runButton.disabled = true;
  show(`<p class="hint">Running the gate. This takes a few seconds.</p>`);
  try {
    const response = await invoke<GateResponse>("run_gate");
    if (response.status === "ready") {
      show(renderSummary(response.summary), renderProvenance(response.summary));
    } else {
      show(renderUnavailable(response.message));
    }
  } catch (error) {
    show(renderUnavailable(String(error)));
  } finally {
    runButton.disabled = false;
  }
}

runButton.addEventListener("click", () => void run());
show(`<p class="hint">Not run yet. Press <kbd>Run gate</kbd>.</p>`);