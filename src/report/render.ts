/**
 * HTML for the published report.
 *
 * Pure: data in, HTML string out, no `document`. Same contract as
 * `src/render.ts`, and for the same reason -- whether a red gate is legible to
 * a person is the thing most likely to be wrong here, so it has to be
 * checkable in a real browser rather than by reading this.
 *
 * Two tables per skill (traps, facts) plus the summary, which is what makes this
 * more than the desktop viewer's screen. The desktop viewer answers "is it
 * green"; a reader of a published run also wants "which claim moved and by how
 * much", and that is the `seal` and the residual columns.
 */

import { esc, ratio } from "../render";
import type { RawFact, RawTrap, Report, SkillView } from "./model";

export interface PublishedMeta {
  published: string | null;
  source_run: string | null;
  redacted_paths: number | null;
  redacted_from_path_fields: number | null;
}

function verdictClass(verdict: string): string {
  if (verdict === "PASS") return "pass";
  if (verdict === "FAIL") return "fail";
  if (verdict === "UNLOCATED") return "unlocated";
  return "unknown";
}

/** A residual is a distance. Zero is worth saying exactly; the rest are not. */
export function residual(value: number | null): string {
  if (value === null || !Number.isFinite(value)) return "&mdash;";
  return esc(Number(value.toFixed(3)));
}

/** `measured` is a string in some traps and a structured value in others. */
/** Long values are clipped for the eye and kept whole for hover. */
const CLIP = 90;

function measured(value: unknown): string {
  if (value === null || value === undefined) return "&mdash;";
  const text =
    typeof value === "string" ? value : JSON.stringify(value) ?? String(value);
  if (text.length <= CLIP) return `<code>${esc(text)}</code>`;
  // The ellipsis says the value is cut; the title carries the rest, so nothing
  // is hidden behind a decision the reader cannot see was made.
  return `<code title="${esc(text)}">${esc(`${text.slice(0, CLIP)}...`)}</code>`;
}

/**
 * Every table sits in its own scroll container.
 *
 * The sticky header and the sticky first column from the modern-web-guidance
 * `responsive-table` guide both need a scrollport to stick to; without a
 * wrapper they stick to the viewport and the header floats free of the table.
 */
function wrapTable(table: string): string {
  return `<div class="table-wrap">${table}</div>`;
}

function figure(label: string, value: string): string {
  return `<div><dt>${esc(label)}</dt><dd>${value}</dd></div>`;
}

/* ------------------------------------------------------------------ traps */

function trapRow(trap: RawTrap, index: number): string {
  const cls = trap.pass ? "pass" : "fail";
  const label = trap.id || `trap ${index + 1}`;
  return (
    `<tr class="${cls}">` +
    `<th scope="row"><code>${esc(label)}</code></th>` +
    `<td>${measured(trap.expected)}</td>` +
    `<td>${measured(trap.measured)}</td>` +
    `<td>${residual(trap.residual)}</td>` +
    `<td><span class="pill ${cls}">${esc(trap.pass ? "holding" : "not holding")}</span></td>` +
    `</tr>`
  );
}

function renderTraps(traps: RawTrap[]): string {
  if (!traps.length) return `<p class="lede">No traps declared for this skill.</p>`;
  return wrapTable(
    `<table class="data traps">` +
    `<caption>Traps &mdash; ${traps.filter((t) => t.pass).length} of ${traps.length} holding</caption>` +
    `<thead><tr>` +
    `<th scope="col">Trap</th><th scope="col">Expected</th>` +
    `<th scope="col">Measured</th><th scope="col">Residual</th>` +
    `<th scope="col">Status</th>` +
    `</tr></thead>` +
    `<tbody>${traps.map(trapRow).join("")}</tbody>` +
    `</table>`
  );
}

/* ------------------------------------------------------------------ facts */

function factRow(fact: RawFact, index: number): string {
  const ok = fact.status === "verified";
  const cls = ok ? "pass" : "fail";
  const label = fact.id || `fact ${index + 1}`;
  return (
    `<tr class="${cls}">` +
    `<th scope="row"><code>${esc(label)}</code></th>` +
    `<td>${esc(fact.claim || fact.detail || "")}</td>` +
    `<td>${residual(fact.residual)}</td>` +
    `<td><span class="pill ${cls}">${esc(fact.status)}</span></td>` +
    `</tr>`
  );
}

function renderFacts(facts: RawFact[]): string {
  if (!facts.length) return `<p class="lede">No facts declared for this skill.</p>`;
  const drifted = facts.filter((f) => f.status !== "verified").length;
  return wrapTable(
    `<table class="data facts">` +
    `<caption>Facts &mdash; ${facts.length - drifted} of ${facts.length} verified` +
    (drifted ? `, ${drifted} drifted` : "") +
    `</caption>` +
    `<thead><tr>` +
    `<th scope="col">Fact</th><th scope="col">Claim</th>` +
    `<th scope="col">Residual</th><th scope="col">Status</th>` +
    `</tr></thead>` +
    `<tbody>${facts.map(factRow).join("")}</tbody>` +
    `</table>`
  );
}

/* ------------------------------------------------------------------- pin */

function renderPin(skill: SkillView): string {
  const pin = skill.pin;
  const cls = pin.status === "PASS" ? "pass" : "fail";
  const rows: string[] = [];
  const cell = (label: string, value: string) =>
    `<tr><th scope="row">${esc(label)}</th><td>${value}</td></tr>`;

  rows.push(
    cell(
      "Status",
      `<span class="pill ${cls}">${esc(pin.status)}</span>` +
        (pin.pinned ? "" : " <span class=\"note\">not pinned</span>"),
    ),
  );
  if (pin.instrument) rows.push(cell("Instrument", `<code>${esc(pin.instrument)}</code>`));
  if (pin.expected_sha256) {
    rows.push(cell("Expected sha256", `<code class="sha">${esc(pin.expected_sha256)}</code>`));
  }
  if (pin.actual_sha256 && pin.actual_sha256 !== pin.expected_sha256) {
    rows.push(cell("Actual sha256", `<code class="sha">${esc(pin.actual_sha256)}</code>`));
  }
  if (pin.expected_bytes !== null || pin.actual_bytes !== null) {
    rows.push(
      cell(
        "Bytes",
        `<code>${esc(pin.expected_bytes ?? undefined)} expected / ${esc(pin.actual_bytes ?? undefined)} actual</code>`,
      ),
    );
  }
  if (pin.detail) rows.push(cell("Detail", esc(pin.detail)));

  return wrapTable(
    `<table class="data pin"><caption>Instrument pin</caption>` +
    `<tbody>${rows.join("")}</tbody></table>`
  );
}

/* ----------------------------------------------------------------- skills */

function renderSkill(skill: SkillView): string {
  const cls = verdictClass(skill.verdict);
  const unsealed = skill.timed_out ? " · timed out" : "";

  const binding = skill.binding ?? ({} as SkillView["binding"]);
  const hygiene = skill.hygiene ?? ({} as SkillView["hygiene"]);
  const extras: string[] = [];
  if (binding.facts !== null && binding.facts !== undefined) {
    extras.push(`${binding.facts} facts in universe`);
  }
  if (binding.unverified_exemptions) {
    extras.push(`${binding.unverified_exemptions} unverified exemptions`);
  }
  if (hygiene.scanned !== null && hygiene.scanned !== undefined) {
    extras.push(`hygiene scanned ${hygiene.scanned}`);
  }
  extras.push(`${skill.runtime_seconds.toFixed(1)}s`);

  const reason = skill.problem
    ? `<p class="why">${esc(skill.problem)}</p>`
    : "";

  return (
    `<details class="skill ${cls}"${cls === "fail" ? " open" : ""}>` +
    `<summary>` +
    `<span class="pill ${cls}">${esc(skill.verdict)}</span>` +
    `<span class="name">${esc(skill.skill)}</span>` +
    `<span class="tally">${ratio(skill.facts_verified, skill.facts.length)} facts · ` +
    `${ratio(skill.traps_holding, skill.traps.length)} traps${unsealed}</span>` +
    `</summary>` +
    `<div class="skill-body">` +
    reason +
    `<p class="meta">${esc(extras.join(" · "))}</p>` +
    (skill.seal
      ? `<p class="meta">seal <code class="sha">${esc(skill.seal)}</code></p>`
      : "") +
    renderPin(skill) +
    renderTraps(skill.traps) +
    renderFacts(skill.facts) +
    `</div></details>`
  );
}

/* ----------------------------------------------------------------- report */

export function renderReport(report: Report, meta: PublishedMeta): string {
  const t = report.totals;
  const cls = verdictClass(report.verdict);
  const headline =
    report.verdict === "PASS"
      ? `PASS &mdash; ${t.passed} of ${t.skills} skills verified`
      : `${esc(report.verdict)} &mdash; ${t.passed} of ${t.skills} skills verified, ${t.failed} failed`;

  const drifted = t.facts_drifted ? ` <em>${t.facts_drifted} drifted</em>` : "";

  const redaction =
    meta.redacted_paths === null || meta.redacted_paths === 0
      ? ""
      : `<p class="note">${esc(meta.redacted_paths)} local path(s) were removed before ` +
        `this page was published (${esc(meta.redacted_from_path_fields ?? 0)} from path ` +
        `fields). No user directory is present in this file.</p>`;

  return (
    `<section class="verdict ${cls}">` +
    `<h2>${headline}</h2>` +
    `<dl class="figures">` +
    figure("Facts", ratio(t.facts_verified, t.facts) + drifted) +
    figure("Traps holding", ratio(t.traps_holding, t.traps)) +
    figure("Hygiene findings", esc(t.hygiene_findings)) +
    figure("Unverified exemptions", esc(t.unverified_exemptions)) +
    figure("Timed out", esc(t.timed_out)) +
    figure("Runtime", `${t.runtime_seconds.toFixed(1)}s`) +
    `</dl>` +
    `<p class="meta">run <code>${esc(report.run)}</code> · payload <code>${esc(report.schema)}</code>` +
    (meta.published ? ` · published <code>${esc(meta.published)}</code>` : "") +
    `</p>` +
    (report.budget_seconds !== null
      ? `<p class="meta">budget ${esc(report.budget_seconds)}s</p>`
      : "") +
    `</section>` +
    `<section class="skills">${report.skills.map(renderSkill).join("")}</section>` +
    redaction
  );
}

/**
 * Shown when the payload contains something that should never have been
 * published, or when there is no payload at all.
 *
 * Same register as the desktop viewer's "no result" panel on purpose: a reader
 * must not be able to mistake a refused page for a passing run.
 */
export function renderRefused(findings: { where: string; value: string }[]): string {
  const list = findings
    .slice(0, 20)
    .map((f) => `<li><code>${esc(f.where)}</code>: <code>${esc(f.value)}</code></li>`)
    .join("");
  return (
    `<section class="unavailable">` +
    `<h2>This report was not published</h2>` +
    `<p class="lede">This is not a verdict. The file still contains a local path, ` +
    `so it was not rendered. Re-run <code>tools/publish_report.py</code> rather than ` +
    `publishing the gate payload directly.</p>` +
    `<ul class="findings">${list}</ul>` +
    `</section>`
  );
}

export function renderMissing(message: string): string {
  return (
    `<section class="unavailable">` +
    `<h2>No report to show</h2>` +
    `<p class="lede">This is not a verdict. No gate run has been published yet.</p>` +
    `<pre>${esc(message)}</pre>` +
    `</section>`
  );
}