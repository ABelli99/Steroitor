const NAME_BONUS = 10;
const CONSECUTIVE_BONUS = 5;
const BOUNDARY_BONUS = 4;
const BOUNDARIES = new Set(["/", "\\", ".", "-", "_", " "]);

/** Punteggio greedy: le lettere del query devono comparire in ordine nel target. */
function subsequenceScore(query: string, target: string, offset: number): number | null {
  let score = 0;
  let from = 0;
  let previous = -2;
  for (const char of query) {
    const found = target.indexOf(char, from);
    if (found === -1) return null;
    if (found === previous + 1) score += CONSECUTIVE_BONUS;
    if (found === 0 || BOUNDARIES.has(target[found - 1])) score += BOUNDARY_BONUS;
    score -= Math.min(found - from, 10) * 0.1;
    previous = found;
    from = found + 1;
  }
  return score - (target.length + offset) * 0.01;
}

export function fuzzyScore(query: string, path: string): number | null {
  const q = query.toLowerCase().replace(/\s+/g, "");
  if (!q) return 0;
  const target = path.toLowerCase();
  const nameStart = Math.max(target.lastIndexOf("/"), target.lastIndexOf("\\")) + 1;

  const inName = subsequenceScore(q, target.slice(nameStart), nameStart);
  if (inName !== null) return inName + NAME_BONUS;
  return subsequenceScore(q, target, 0);
}

export function fuzzyFilter(query: string, paths: string[], limit: number): string[] {
  const scored: Array<{ path: string; score: number }> = [];
  for (const path of paths) {
    const score = fuzzyScore(query, path);
    if (score !== null) scored.push({ path, score });
  }
  return scored
    .sort((a, b) => b.score - a.score)
    .slice(0, limit)
    .map((item) => item.path);
}
