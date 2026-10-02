/**
 * The published report's data model.
 *
 * The gate emits a raw `elohim.gate/1` payload: per-skill `verdict`, `traps`,
 * `facts`, `instrument_pin` and a `seal`. The desktop viewer never sees that
 * shape -- `src-tauri/src/gate.rs` flattens it into the `GateSummary` that
 * `src/render.ts` consumes. A static page has no Rust beside it, so the same
 * flattening happens here, in TypeScript.
 *
 * Every number in this file is read out of the payload. Nothing is inferred
 * that the payload does not state. In particular `elohim.gate/1` carries no exit
 * code, no command line and no stderr, so this model has no field for them and
 * the renderer never prints one.
 */

export interface RawTrap {
  id: string;
  expected: unknown;
  measured: unknown;
  pass: boolean;
  residual: number | null;
  why: string | null;
  detail: string | null;
}

export interface RawFact {
  id: string;
  claim: string;
  detail: string | null;
  status: "verified" | "drifted" | string;
  residual: number | null;
}

export interface RawPin {
  pinned: boolean;
  source: string | null;
  status: string;
  path: string | null;
  expected_sha256: string | null;
  actual_sha256: string | null;
  expected_bytes: number | null;
  actual_bytes: number | null;
  detail: string | null;
}

export interface RawBinding {
  check: string | null;
  status: string;
  ok: boolean;
  returncode: number | null;
  runtime_seconds: number | null;
  facts: number | null;
  ledgers: number | null;
  declared_exemptions: number | null;
  unverified_exemptions: number | null;
  failures: unknown[];
  id_failures: unknown[];
}

export interface RawHygiene {
  ok: boolean;
  clean: boolean;
  status: string;
  returncode: number | null;
  runtime_seconds: number | null;
  scanned: number | null;
  findings: unknown[];
}

export interface RawSkill {
  skill: string;
  verdict: string;
  run: string;
  schema: string;
  seal: string | null;
  timed_out: boolean;
  timed_out_phase: string | null;
  instrument: string | null;
  instrument_error: string | null;
  instrument_source: string;
  instrument_pin: RawPin;
  runtime: Record<string, number>;
  stdout_tail: string | null;
  claim_binding: RawBinding;
  hygiene: RawHygiene;
  traps: RawTrap[];
  facts: RawFact[];
}

export interface RawPayload {
  schema: string;
  run: string;
  budget_seconds: number | null;
  fail_under: number | null;
  skills: RawSkill[];
}

export interface PinView {
  status: string;
  pinned: boolean;
  source: string | null;
  /** File name only. The directory is removed by `tools/publish_report.py`. */
  instrument: string | null;
  expected_sha256: string | null;
  actual_sha256: string | null;
  expected_bytes: number | null;
  actual_bytes: number | null;
  detail: string | null;
}

export interface SkillView {
  skill: string;
  verdict: string;
  seal: string | null;
  run: string;
  timed_out: boolean;
  instrument_source: string;
  instrument_error: string | null;
  pin: PinView;
  binding: RawBinding;
  hygiene: RawHygiene;
  traps: RawTrap[];
  facts: RawFact[];
  facts_verified: number;
  traps_holding: number;
  runtime_seconds: number;
  /** Why this skill is not green, in the payload's own words. Empty when it is. */
  problem: string;
}

export interface Report {
  schema: string;
  run: string;
  verdict: string;
  budget_seconds: number | null;
  skills: SkillView[];
  totals: {
    skills: number;
    passed: number;
    failed: number;
    unlocated: number;
    facts: number;
    facts_verified: number;
    facts_drifted: number;
    traps: number;
    traps_holding: number;
    hygiene_findings: number;
    unverified_exemptions: number;
    timed_out: number;
    runtime_seconds: number;
  };
}

function num(value: unknown): number | null {
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

function list<T>(value: unknown): T[] {
  return Array.isArray(value) ? (value as T[]) : [];
}

/** File name of a path, without asserting anything about the path itself. */
export function baseName(path: string | null): string | null {
  if (!path) return null;
  const cut = path.replace(/\\/g, "/").lastIndexOf("/");
  return cut === -1 ? path : path.slice(cut + 1);
}

/**
 * Why a skill is not green, assembled from the payload rather than invented.
 *
 * A FAIL verdict in `elohim.gate/1` has no message of its own -- it is a
 * summary of the blocks below it. So the reason is rebuilt from those blocks in
 * the order the gate itself would have found them.
 */
export function problem(skill: RawSkill): string {
  const reasons: string[] = [];
  if (skill.instrument_error) reasons.push(`instrument error: ${skill.instrument_error}`);
  if (skill.timed_out) {
    reasons.push(`timed out${skill.timed_out_phase ? ` in ${skill.timed_out_phase}` : ""}`);
  }
  if (skill.instrument_pin?.status && skill.instrument_pin.status !== "PASS") {
    reasons.push(skill.instrument_pin.detail || `pin ${skill.instrument_pin.status}`);
  }
  const drift = list<RawFact>(skill.facts).filter((f) => f.status !== "verified");
  if (drift.length) reasons.push(`${drift.length} fact${drift.length === 1 ? "" : "s"} drifted`);
  const broken = list<RawTrap>(skill.traps).filter((t) => t.pass === false);
  if (broken.length) {
    reasons.push(`${broken.length} trap${broken.length === 1 ? "" : "s"} not holding`);
  }
  const binding = skill.claim_binding;
  if (binding && (binding.ok === false || (binding.status && binding.status !== "OK"))) {
    const failures = list(binding.failures).length + list(binding.id_failures).length;
    reasons.push(
      failures
        ? `claim binding failed: ${failures} failure${failures === 1 ? "" : "s"}`
        : "claim binding not OK",
    );
  }
  if (list<unknown>(skill.hygiene?.findings).length) {
    reasons.push(`${list<unknown>(skill.hygiene.findings).length} hygiene finding(s)`);
  }
  return reasons.join(" · ");
}

function view(skill: RawSkill): SkillView {
  const pin = skill.instrument_pin ?? ({} as RawPin);
  return {
    skill: skill.skill,
    verdict: skill.verdict,
    seal: skill.seal ?? null,
    run: skill.run,
    timed_out: skill.timed_out === true,
    instrument_source: skill.instrument_source ?? "unknown",
    instrument_error: skill.instrument_error ?? null,
    pin: {
      status: pin.status ?? "UNKNOWN",
      pinned: pin.pinned === true,
      source: pin.source ?? null,
      instrument: baseName(skill.instrument ?? pin.path ?? null),
      expected_sha256: pin.expected_sha256 ?? null,
      actual_sha256: pin.actual_sha256 ?? null,
      expected_bytes: pin.expected_bytes ?? null,
      actual_bytes: pin.actual_bytes ?? null,
      detail: pin.detail ?? null,
    },
    binding: skill.claim_binding ?? ({} as RawBinding),
    hygiene: skill.hygiene ?? ({} as RawHygiene),
    traps: list<RawTrap>(skill.traps),
    facts: list<RawFact>(skill.facts),
    facts_verified: list<RawFact>(skill.facts).filter((f) => f.status === "verified").length,
    traps_holding: list<RawTrap>(skill.traps).filter((t) => t.pass === true).length,
    runtime_seconds: num(skill.runtime?.total) ?? 0,
    problem: problem(skill),
  };
}

/**
 * The overall verdict, from the per-skill verdicts.
 *
 * `elohim.gate/1` has no top-level verdict, so this is the same reduction the
 * gate applies: any FAIL fails the run, any UNLOCATED with no FAIL is reported
 * as UNLOCATED rather than quietly counted as a pass.
 */
export function overallVerdict(skills: { verdict: string }[]): string {
  if (skills.some((s) => s.verdict === "FAIL")) return "FAIL";
  if (skills.some((s) => s.verdict === "UNLOCATED")) return "UNLOCATED";
  return "PASS";
}

export function toReport(payload: RawPayload): Report {
  const skills = list<RawSkill>(payload.skills).map(view);
  const facts = skills.reduce((n, s) => n + s.facts.length, 0);
  return {
    schema: payload.schema,
    run: payload.run,
    verdict: overallVerdict(skills),
    budget_seconds: num(payload.budget_seconds),
    skills,
    totals: {
      skills: skills.length,
      passed: skills.filter((s) => s.verdict === "PASS").length,
      failed: skills.filter((s) => s.verdict === "FAIL").length,
      unlocated: skills.filter((s) => s.verdict === "UNLOCATED").length,
      facts,
      facts_verified: skills.reduce((n, s) => n + s.facts_verified, 0),
      facts_drifted: skills.reduce(
        (n, s) => n + s.facts.filter((f) => f.status !== "verified").length,
        0,
      ),
      traps: skills.reduce((n, s) => n + s.traps.length, 0),
      traps_holding: skills.reduce((n, s) => n + s.traps_holding, 0),
      hygiene_findings: skills.reduce((n, s) => n + list(s.hygiene?.findings).length, 0),
      unverified_exemptions: skills.reduce(
        (n, s) => n + (num(s.binding?.unverified_exemptions) ?? 0),
        0,
      ),
      timed_out: skills.filter((s) => s.timed_out).length,
      runtime_seconds: skills.reduce((n, s) => n + s.runtime_seconds, 0),
    },
  };
}