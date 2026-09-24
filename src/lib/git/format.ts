const relative = new Intl.RelativeTimeFormat("it", { numeric: "auto" });

const UNITS: Array<[Intl.RelativeTimeFormatUnit, number]> = [
  ["year", 31_536_000],
  ["month", 2_592_000],
  ["week", 604_800],
  ["day", 86_400],
  ["hour", 3_600],
  ["minute", 60],
];

export function relativeTime(timestamp: number, now = Date.now() / 1000): string {
  const elapsed = timestamp - now;
  for (const [unit, seconds] of UNITS) {
    if (Math.abs(elapsed) >= seconds) return relative.format(Math.round(elapsed / seconds), unit);
  }
  return "adesso";
}

export const absoluteTime = (timestamp: number) =>
  new Date(timestamp * 1000).toLocaleString("it-IT", { dateStyle: "medium", timeStyle: "short" });

/** "HEAD -> main" diventa "main", i tag perdono il prefisso "tag: ". */
export function refLabel(ref: string): { label: string; kind: "head" | "tag" | "branch" } {
  if (ref.startsWith("HEAD -> ")) return { label: ref.slice(8), kind: "head" };
  if (ref.startsWith("tag: ")) return { label: ref.slice(5), kind: "tag" };
  return { label: ref, kind: ref === "HEAD" ? "head" : "branch" };
}
