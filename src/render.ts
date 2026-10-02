/**
 * Rendering for the gate payload.
 *
 * Every function here is pure: data in, HTML string out, no `document` and no
 * Tauri. That is what lets the one thing most likely to be wrong -- how a red
 * gate is actually presented to a person -- be checked by rendering it in a
 * real browser rather than by reading it.
 *
 * The two invocations are `invoke("run_gate")` and `panel.innerHTML`. The first
 * is covered by `cargo test --test real_gate`; this file is the second half.
 */

export interface SkillRow {
  skill: string;
  verdict: string;
  pin_status: string;
  pin_sha: string;
  pin_detail: string;
  instrument_source: string;
  facts_verified: number;
  facts_total: number;
  traps_holding: number;
  traps_total: number;
  timed_out: boolean;
  problem: string;
}

export interface GateSummary {
  schema: string;
  verdict: string;
  exit_code: number;
  skills: number;
  passed: number;
  failed: number;
  unlocated: number;
  facts_verified: number;
  facts: number;
  facts_drifted: number;
  traps_holding: number;
  traps: number;
  hygiene_findings: number;
  unbound_claims: number;
  timed_out: number;
  runtime_seconds: number;
  rows: SkillRow[];
  stderr_tail: string;
  command: string;
}

export type GateResponse =
  | { status: "ready"; summary: GateSummary }
  | { status: "unavailable"; message: string };

/** Escape for interpolation. Every string here originates in a subprocess. */
export function esc(value: string | number | undefined): string {
  return String(value ?? "")
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

/**
 * `81/81`, or an em dash when there is nothing to compare.
 *
 * A dash rather than `0/0`: "0 of 0 verified" reads as a perfect score to most
 * people, when it usually means nothing was measured at all.
 */
export function ratio(holding: number, total: number): string {
  return total === 0 ? "&mdash;" : esc(`${holding}/${total}`);
}

function verdictClass(verdict: string): string {
  switch (verdict) {
    case "PASS":
      return "pass";
    case "FAIL":
      return "fail";
    case "UNLOCATED":
      return "unlocated";
    default:
      return "unknown";
  }
}

/** Why this skill is not green, in the gate's own words. Empty when it is. */
function why(row: SkillRow): string {
  const reasons: string[] = [];
  if (row.problem) reasons.push(row.problem);
  if (row.pin_status !== "PASS") {
    reasons.push(row.pin_detail || `pin ${row.pin_status}`);
  }
  return reasons.join(" · ");
}

function renderRow(row: SkillRow): string {
  const main =
    `<tr class="${verdictClass(row.verdict)}">` +
    `<th scope="row">${esc(row.skill)}</th>` +
    `<td><span class="pill ${verdictClass(row.verdict)}">${esc(row.verdict)}</span></td>` +
    `<td class="pin ${verdictClass(row.pin_status)}">${esc(row.pin_status)}${
      row.pin_sha ? ` <code>${esc(row.pin_sha)}</code>` : ""
    }</td>` +
    `<td>${ratio(row.facts_verified, row.facts_total)}</td>` +
    `<td>${ratio(row.traps_holding, row.traps_total)}</td>` +
    `<td>${esc(row.instrument_source)}</td>` +
    `</tr>`;

  // The reason sits in its own row directly beneath the skill, as a cell that
  // spans the table. It cannot be a sibling <p> inside <tbody>: that is invalid
  // and browsers hoist it out of the table entirely, which put every reason in
  // one strip above the first row, detached from the skill it explained.
  const reason = why(row);
  if (!reason) return main;
  return main + `<tr class="why-row"><td colspan="6" class="why">${esc(reason)}</td></tr>`;
}

function figure(label: string, value: string): string {
  return `<div><dt>${esc(label)}</dt><dd>${value}</dd></div>`;
}

export function renderSummary(summary: GateSummary): string {
  const verdict = esc(summary.verdict);
  const headline =
    summary.verdict === "PASS"
      ? `PASS &mdash; ${summary.passed} of ${summary.skills} skills verified`
      : `${verdict} &mdash; ${summary.passed} of ${summary.skills} skills verified, ${summary.failed} failed`;

  const drifted = summary.facts_drifted
    ? ` <em>${summary.facts_drifted} drifted</em>`
    : "";

  const stderr = summary.stderr_tail.trim()
    ? `<details class="stderr"><summary>Gate stderr</summary><pre>${esc(summary.stderr_tail)}</pre></details>`
    : "";

  return (
    `<section class="verdict ${verdictClass(summary.verdict)}">` +
    `<h2>${headline}</h2>` +
    `<dl class="figures">` +
    figure("Facts", ratio(summary.facts_verified, summary.facts) + drifted) +
    figure("Traps holding", ratio(summary.traps_holding, summary.traps)) +
    figure("Unlocated", esc(summary.unlocated)) +
    figure("Hygiene findings", esc(summary.hygiene_findings)) +
    figure("Unbound claims", esc(summary.unbound_claims)) +
    figure("Timed out", esc(summary.timed_out)) +
    figure("Runtime", `${summary.runtime_seconds.toFixed(1)}s`) +
    figure("Exit code", esc(summary.exit_code)) +
    `</dl></section>` +
    `<table class="skills">` +
    `<caption>Per-skill results</caption>` +
    `<thead><tr>` +
    `<th scope="col">Skill</th><th scope="col">Verdict</th>` +
    `<th scope="col">Instrument pin</th><th scope="col">Facts</th>` +
    `<th scope="col">Traps</th><th scope="col">Source</th>` +
    `</tr></thead>` +
    `<tbody>${summary.rows.map(renderRow).join("")}</tbody>` +
    `</table>` +
    stderr
  );
}

/**
 * The command with every rooted path reduced to its file name.
 *
 * The header used to print `command` verbatim, which put the author's home
 * directory into every screenshot of this window and therefore into the public
 * repository. The flags are the part worth reading, and they survive intact.
 *
 * Splitting is on whitespace, so a path containing a space would be cut in
 * half. The gate's own path has none, and a truncated path is still a better
 * outcome than publishing someone's home directory.
 */
export function reduceCommand(command: string): string {
  return command
    .split(/\s+/)
    .filter((token) => token.length > 0)
    .map((token) =>
      token.startsWith("/") ? token.slice(token.lastIndexOf("/") + 1) : token
    )
    .join(" ");
}

export function renderProvenance(summary: GateSummary): string {
  const command = reduceCommand(summary.command);
  return `${esc(command)} &middot; payload <code>${esc(summary.schema)}</code>`;
}

/**
 * The panel shown when there is no verdict to display.
 *
 * Deliberately plain and deliberately explicit that this is not a result. A
 * missing gate, an old interpreter, and a payload from an unpinned contract are
 * all cases where showing a confident-looking table would be a lie; saying "no
 * result" in the same visual register as a result is the only honest option.
 */
export function renderUnavailable(message: string): string {
  return (
    `<section class="unavailable">` +
    `<h2>No gate result to show</h2>` +
    `<p class="lede">This is not a verdict. Nothing was measured, so there is ` +
    `no claim here about whether the skills are healthy.</p>` +
    `<pre>${esc(message)}</pre>` +
    `<button id="retry" type="button">Try again</button>` +
    `</section>`
  );
}