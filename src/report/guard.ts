/**
 * Fail-closed guard against publishing a local path.
 *
 * `tools/publish_report.py` is what actually removes local paths, once, before
 * anything is written. This module is deliberately redundant and deliberately
 * *stricter*: it does not try to scrub anything, it only refuses to render.
 * That asymmetry is the point. A scrubber that is too narrow leaks silently and
 * nobody notices; a guard that is too narrow shows a "cannot publish" page,
 * which is a loud, correct, cheap failure.
 *
 * The pattern therefore lists the prefixes a scrubber would have to reason
 * about instead of trying to reason about all paths, which is the thing a
 * regular expression cannot do reliably.
 */

const PATH_PREFIX = /(^|[^\w.~-])(\/(?:home|tmp|root|Users|var|etc|opt|usr|nix|private|mnt|srv|run|proc|dev|Library|Applications)\/)/;

export interface Finding {
  where: string;
  value: string;
}

function walk(node: unknown, trail: string, found: Finding[]): void {
  if (typeof node === "string") {
    if (PATH_PREFIX.test(node)) found.push({ where: trail, value: node });
    return;
  }
  if (Array.isArray(node)) {
    node.forEach((item, i) => walk(item, `${trail}[${i}]`, found));
    return;
  }
  if (node && typeof node === "object") {
    for (const [key, value] of Object.entries(node)) walk(value, `${trail}.${key}`, found);
  }
}

export function findLocalPaths(payload: unknown): Finding[] {
  const found: Finding[] = [];
  walk(payload, "payload", found);
  return found;
}

/** True when `payload` is safe to put in front of a reader. */
export function isPublishable(payload: unknown): boolean {
  return findLocalPaths(payload).length === 0;
}