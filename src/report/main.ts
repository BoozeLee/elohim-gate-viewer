/**
 * Bootstrap for the published report page.
 *
 * There is no Tauri here and no button. The page fetches one file that
 * `tools/publish_report.py` wrote, refuses to render it if a local path is
 * still in it, and renders it otherwise.
 */

import { findLocalPaths } from "./guard";
import { toReport, type RawPayload } from "./model";
import { renderMissing, renderRefused, renderReport, type PublishedMeta } from "./render";
import "./report.css";

const DATA_URL = "./data/latest.json";

function mount(html: string): void {
  const panel = document.getElementById("panel");
  if (panel) panel.innerHTML = html;
  document.documentElement.dataset.state = "ready";
}

async function main(): Promise<void> {
  let response: Response;
  try {
    response = await fetch(DATA_URL, { cache: "no-cache" });
  } catch (err) {
    mount(renderMissing(`could not fetch ${DATA_URL}: ${String(err)}`));
    return;
  }

  if (!response.ok) {
    mount(renderMissing(`${DATA_URL} returned ${response.status} ${response.statusText}`));
    return;
  }

  let document_: PublishedMeta & { payload: RawPayload };
  try {
    document_ = (await response.json()) as typeof document_;
  } catch (err) {
    mount(renderMissing(`${DATA_URL} is not valid JSON: ${String(err)}`));
    return;
  }

  // Fail closed. The publisher already removed paths; if one is still here then
  // either it slipped through or the file was placed by hand, and in both cases
  // the honest thing to show a reader is a refusal, not a green run.
  const findings = findLocalPaths(document_);
  if (findings.length) {
    mount(renderRefused(findings));
    return;
  }

  mount(
    renderReport(toReport(document_.payload), {
      published: document_.published ?? null,
      source_run: document_.source_run ?? null,
      redacted_paths: document_.redacted_paths ?? null,
      redacted_from_path_fields: document_.redacted_from_path_fields ?? null,
    }),
  );
}

void main();